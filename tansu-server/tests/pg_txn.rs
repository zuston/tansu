// Copyright ⓒ 2024 Peter Morgan <peter.james.morgan@gmail.com>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as
// published by the Free Software Foundation, either version 3 of the
// License, or (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use bytes::Bytes;
use rand::{prelude::*, thread_rng};
use tansu_kafka_sans_io::{
    add_partitions_to_txn_request::AddPartitionsToTxnTopic,
    broker_registration_request::Listener,
    create_topics_request::CreatableTopic,
    join_group_request::JoinGroupRequestProtocol,
    join_group_response::JoinGroupResponseMember,
    offset_fetch_request::OffsetFetchRequestTopic,
    offset_fetch_response::{OffsetFetchResponsePartition, OffsetFetchResponseTopic},
    record::{inflated, Record},
    sync_group_request::SyncGroupRequestAssignment,
    txn_offset_commit_request::{TxnOffsetCommitRequestPartition, TxnOffsetCommitRequestTopic},
    txn_offset_commit_response::{TxnOffsetCommitResponsePartition, TxnOffsetCommitResponseTopic},
    ErrorCode,
};
use tansu_server::{coordinator::group::administrator::Controller, Error, Result};
use tansu_storage::{
    BrokerRegistationRequest, Storage, Topition, TxnAddPartitionsRequest, TxnOffsetCommitRequest,
};
use tracing::{debug, subscriber::DefaultGuard};
use uuid::Uuid;

mod common;

fn init_tracing() -> Result<DefaultGuard> {
    use std::{fs::File, sync::Arc, thread};

    use tracing::Level;

    Ok(tracing::subscriber::set_default(
        tracing_subscriber::fmt()
            .with_level(true)
            .with_line_number(true)
            .with_max_level(Level::DEBUG)
            .with_writer(
                thread::current()
                    .name()
                    .ok_or(Error::Message(String::from("unnamed thread")))
                    .and_then(|name| {
                        File::create(format!("../logs/{}/{name}.log", env!("CARGO_PKG_NAME")))
                            .map_err(Into::into)
                    })
                    .map(Arc::new)?,
            )
            .finish(),
    ))
}

#[tokio::test]
async fn simple_txn() -> Result<()> {
    let _guard = init_tracing()?;

    let mut rng = thread_rng();

    let cluster_id = Uuid::now_v7();
    let broker_id = rng.gen_range(0..i32::MAX);
    let incarnation_id = Uuid::now_v7();

    debug!(?cluster_id, ?broker_id, ?incarnation_id);

    let mut sc = common::storage_container(cluster_id, broker_id)?;

    let port = rng.gen_range(1024..u16::MAX);
    let security_protocol = rng.gen_range(0..i16::MAX);

    let broker_registration = BrokerRegistationRequest {
        broker_id,
        cluster_id: cluster_id.into(),
        incarnation_id,
        listeners: vec![Listener {
            name: "broker".into(),
            host: "test.local".into(),
            port,
            security_protocol,
        }],
        features: vec![],
        rack: None,
    };

    sc.register_broker(broker_registration).await?;

    let input_topic_name: String = common::alphanumeric_string(15);
    debug!(?input_topic_name);

    let num_partitions = 6;
    let replication_factor = 0;
    let assignments = Some([].into());
    let configs = Some([].into());

    let input_topic_id = sc
        .create_topic(
            CreatableTopic {
                name: input_topic_name.clone(),
                num_partitions,
                replication_factor,
                assignments,
                configs,
            },
            false,
        )
        .await?;
    debug!(?input_topic_id);

    let partition_index = rng.gen_range(0..num_partitions);
    let topition = Topition::new(input_topic_name.clone(), partition_index);
    let records = 50;

    for n in 0..records {
        let value = format!("Lorem ipsum dolor sit amet: {n}");

        let batch = inflated::Batch::builder()
            .record(Record::builder().value(Bytes::copy_from_slice(value.as_bytes()).into()))
            .build()
            .and_then(TryInto::try_into)
            .inspect(|deflated| debug!(?deflated))?;

        _ = sc
            .produce(&topition, batch)
            .await
            .inspect(|offset| debug!(?offset))?;
    }

    let mut controller = Controller::with_storage(sc.clone())?;

    let session_timeout_ms = 45_000;
    let rebalance_timeout_ms = Some(300_000);
    let group_instance_id = None;
    let reason = None;

    let group_id: String = common::alphanumeric_string(15);
    debug!(?group_id);

    const CLIENT_ID: &str = "console-consumer";
    const RANGE: &str = "range";
    const COOPERATIVE_STICKY: &str = "cooperative-sticky";

    const PROTOCOL_TYPE: &str = "consumer";

    let first_member_range_meta = Bytes::from_static(b"first_member_range_meta_01");
    let first_member_sticky_meta = Bytes::from_static(b"first_member_sticky_meta_01");

    let protocols = [
        JoinGroupRequestProtocol {
            name: RANGE.into(),
            metadata: first_member_range_meta.clone(),
        },
        JoinGroupRequestProtocol {
            name: COOPERATIVE_STICKY.into(),
            metadata: first_member_sticky_meta,
        },
    ];

    let member_id_required = common::join_group(
        &mut controller,
        Some(CLIENT_ID),
        group_id.as_str(),
        session_timeout_ms,
        rebalance_timeout_ms,
        "",
        group_instance_id,
        PROTOCOL_TYPE,
        Some(&protocols[..]),
        reason,
    )
    .await?;

    assert_eq!(ErrorCode::MemberIdRequired, member_id_required.error_code);
    assert_eq!("consumer", member_id_required.protocol_type);
    assert_eq!("", member_id_required.protocol_name);
    assert!(member_id_required.leader.is_empty());
    assert!(member_id_required.member_id.starts_with(CLIENT_ID));
    assert_eq!(0, member_id_required.members.len());

    let join_response = common::join_group(
        &mut controller,
        Some(CLIENT_ID),
        group_id.as_str(),
        session_timeout_ms,
        rebalance_timeout_ms,
        member_id_required.member_id.as_str(),
        group_instance_id,
        PROTOCOL_TYPE,
        Some(&protocols[..]),
        reason,
    )
    .await?;

    assert_eq!(ErrorCode::None, join_response.error_code);
    assert_eq!(0, join_response.generation_id);
    assert_eq!(PROTOCOL_TYPE, join_response.protocol_type);
    assert_eq!(RANGE, join_response.protocol_name);
    assert_eq!(member_id_required.member_id.as_str(), join_response.leader);
    assert_eq!(
        vec![JoinGroupResponseMember {
            member_id: member_id_required.member_id.clone(),
            group_instance_id: None,
            metadata: first_member_range_meta.clone(),
        }],
        join_response.members
    );

    let member_id = member_id_required.member_id.clone();
    debug!(?member_id);

    let first_member_assignment_01 = Bytes::from_static(b"assignment_01");

    let assignments = [SyncGroupRequestAssignment {
        member_id: member_id.clone(),
        assignment: first_member_assignment_01.clone(),
    }];

    let sync_response = common::sync_group(
        &mut controller,
        group_id.as_str(),
        join_response.generation_id,
        member_id.as_str(),
        group_instance_id,
        PROTOCOL_TYPE,
        RANGE,
        &assignments,
    )
    .await?;
    assert_eq!(ErrorCode::None, sync_response.error_code);
    assert_eq!(PROTOCOL_TYPE, sync_response.protocol_type);
    assert_eq!(RANGE, sync_response.protocol_name);
    assert_eq!(first_member_assignment_01, sync_response.assignment);

    assert_eq!(
        common::HeartbeatResponse {
            error_code: ErrorCode::None,
        },
        common::heartbeat(
            &mut controller,
            group_id.as_str(),
            join_response.generation_id,
            &member_id,
            group_instance_id
        )
        .await?
    );

    let transaction_id: String = common::alphanumeric_string(10);
    debug!(?transaction_id);

    let transaction_timeout_ms = 10_000;

    let producer = sc
        .init_producer(
            Some(transaction_id.as_str()),
            transaction_timeout_ms,
            Some(-1),
            Some(-1),
        )
        .await?;
    debug!(?producer);

    let txn_add_partitions = TxnAddPartitionsRequest::VersionZeroToThree {
        transaction_id: transaction_id.clone(),
        producer_id: producer.id,
        producer_epoch: producer.epoch,
        topics: vec![AddPartitionsToTxnTopic {
            name: input_topic_name.clone(),
            partitions: Some((0..num_partitions).collect()),
        }],
    };

    let txn_add_partitions_response = sc.txn_add_partitions(txn_add_partitions).await?;
    debug!(?txn_add_partitions_response);
    assert_eq!(1, txn_add_partitions_response.zero_to_three().len());
    assert_eq!(
        input_topic_name,
        txn_add_partitions_response.zero_to_three()[0].name
    );

    assert_eq!(
        ErrorCode::None,
        sc.txn_add_offsets(
            transaction_id.as_str(),
            producer.id,
            producer.epoch,
            group_id.as_str(),
        )
        .await?
    );

    let offsets = TxnOffsetCommitRequest {
        transaction_id: transaction_id.clone(),
        group_id: group_id.clone(),
        producer_id: producer.id,
        producer_epoch: producer.epoch,
        generation_id: Some(join_response.generation_id),
        member_id: Some(member_id),
        group_instance_id: group_instance_id.map(|group_instance_id| group_instance_id.to_owned()),
        topics: [TxnOffsetCommitRequestTopic {
            name: input_topic_name.clone(),
            partitions: Some(
                [TxnOffsetCommitRequestPartition {
                    partition_index,
                    committed_offset: 0,
                    committed_leader_epoch: Some(-1),
                    committed_metadata: None,
                }]
                .into(),
            ),
        }]
        .into(),
    };

    assert_eq!(
        vec![TxnOffsetCommitResponseTopic {
            name: input_topic_name.clone(),
            partitions: Some(
                [TxnOffsetCommitResponsePartition {
                    partition_index,
                    error_code: ErrorCode::None.into(),
                }]
                .into(),
            ),
        }],
        sc.txn_offset_commit(offsets).await?
    );

    assert_eq!(
        common::OffsetFetchResponse {
            topics: [OffsetFetchResponseTopic {
                name: input_topic_name.clone(),
                partitions: Some(
                    [OffsetFetchResponsePartition {
                        partition_index,
                        committed_offset: -1,
                        committed_leader_epoch: None,
                        metadata: None,
                        error_code: 0
                    }]
                    .into()
                )
            }]
            .into(),
            error_code: ErrorCode::None
        },
        common::offset_fetch(
            &mut controller,
            group_id.as_str(),
            &[OffsetFetchRequestTopic {
                name: input_topic_name.clone(),
                partition_indexes: Some([partition_index].into()),
            }],
        )
        .await?
    );

    assert_eq!(
        ErrorCode::None,
        sc.txn_end(transaction_id.as_str(), producer.id, producer.epoch, true)
            .await?
    );

    assert_eq!(
        common::OffsetFetchResponse {
            topics: [OffsetFetchResponseTopic {
                name: input_topic_name.clone(),
                partitions: Some(
                    [OffsetFetchResponsePartition {
                        partition_index,
                        // TODO: this should be 0...
                        committed_offset: -1,
                        committed_leader_epoch: None,
                        metadata: None,
                        error_code: 0
                    }]
                    .into()
                )
            }]
            .into(),
            error_code: ErrorCode::None
        },
        common::offset_fetch(
            &mut controller,
            group_id.as_str(),
            &[OffsetFetchRequestTopic {
                name: input_topic_name.clone(),
                partition_indexes: Some([partition_index].into()),
            }],
        )
        .await?
    );

    Ok(())
}
