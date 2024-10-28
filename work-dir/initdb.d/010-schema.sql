-- -*- mode: sql; sql-product: postgres; -*-
-- Copyright ⓒ 2024 Peter Morgan <peter.james.morgan@gmail.com>
--
-- This program is free software: you can redistribute it and/or modify
-- it under the terms of the GNU Affero General Public License as
-- published by the Free Software Foundation, either version 3 of the
-- License, or (at your option) any later version.
--
-- This program is distributed in the hope that it will be useful,
-- but WITHOUT ANY WARRANTY; without even the implied warranty of
-- MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
-- GNU Affero General Public License for more details.
--
-- You should have received a copy of the GNU Affero General Public License
-- along with this program.  If not, see <https://www.gnu.org/licenses/>.

begin;

create table cluster (
  id int generated always as identity primary key,
  name text not null,
  unique (name),
  last_updated timestamp default current_timestamp not null,
  created_at timestamp default current_timestamp not null
);

create table broker (
  id int generated always as identity primary key,

  cluster int references cluster(id) not null,
  node int not null,
  unique (cluster, node),

  rack text,
  incarnation uuid not null,
  last_updated timestamp default current_timestamp not null,
  created_at timestamp default current_timestamp not null
);

create table listener (
  id int generated always as identity primary key,

  broker int references broker(id) not null,
  name text not null,
  unique (broker, name),

  host text not null,
  port int not null,
  last_updated timestamp default current_timestamp not null,
  created_at timestamp default current_timestamp not null
);

create table topic (
  id int generated always as identity primary key,

  cluster int references cluster(id) not null,
  name text not null,
  unique (cluster, name),

  uuid uuid default gen_random_uuid(),
  partitions int not null,
  replication_factor int not null,
  is_internal bool default false not null,
  last_updated timestamp default current_timestamp not null,
  created_at timestamp default current_timestamp not null
);

create table topition (
  id int generated always as identity primary key,

  topic int references topic(id),
  partition int,
  unique (topic, partition),

  last_updated timestamp default current_timestamp not null,
  created_at timestamp default current_timestamp not null
);


create table watermark (
  id int generated always as identity primary key,

  topition int references topition(id),
  unique(topition),

  low bigint,
  high bigint,
  stable bigint,

  last_updated timestamp default current_timestamp not null,
  created_at timestamp default current_timestamp not null
);

create table topic_configuration (
  id int generated always as identity primary key,

  topic int references topic(id),
  name text not null,
  unique (topic, name),

  value text,
  last_updated timestamp default current_timestamp not null,
  created_at timestamp default current_timestamp not null
);

create table record (
  id bigint generated always as identity primary key,

  topition int references topition(id),
  offset_id bigint not null,
  unique (topition, offset_id),

  producer_id bigint,
  sequence int,
  timestamp timestamp,
  k bytea,
  v bytea,
  last_updated timestamp default current_timestamp not null,
  created_at timestamp default current_timestamp not null
);

create table header (
  id bigint generated always as identity primary key,

  record int references record(id),
  k bytea,
  unique (record, k),

  v bytea,

  last_updated timestamp default current_timestamp not null,
  created_at timestamp default current_timestamp not null
);

create table consumer_group (
  id int generated always as identity primary key,

  cluster int references cluster(id) not null,
  name text not null,
  unique (cluster, name),

  e_tag uuid not null,
  detail json not null,
  last_updated timestamp default current_timestamp not null,
  created_at timestamp default current_timestamp not null
);


create table consumer_offset (
  id int generated always as identity primary key,

  consumer_group int references consumer_group(id),
  topition int references topition(id),
  unique (consumer_group, topition),

  committed_offset bigint,
  leader_epoch int,
  timestamp timestamp,
  metadata text,
  last_updated timestamp default current_timestamp not null,
  created_at timestamp default current_timestamp not null
);

-- InitProducerIdRequest
--
create table producer (
  id bigint generated by default as identity primary key,
  epoch int default 0 not null,
  unique (id, epoch),

  cluster int references cluster(id) not null,
  last_updated timestamp default current_timestamp not null,
  created_at timestamp default current_timestamp not null
);

-- InitProducerIdRequest (txn detail)
--
create table txn (
  id int generated always as identity primary key,

  cluster int references cluster(id),
  name text,
  unique (cluster, name),

  transaction_timeout_ms int not null,
  producer bigint references producer(id),
  last_updated timestamp default current_timestamp not null,
  created_at timestamp default current_timestamp not null
);

-- AddPartitionsToTxnRequest
--
create table txn_partition (
  id int generated always as identity primary key,

  transaction int references txn(id),
  topition int references topition(id),
  unique (transaction, topition),

  last_updated timestamp default current_timestamp not null,
  created_at timestamp default current_timestamp not null
);

create table txn_offset (
  id int generated always as identity primary key,

  transaction int references txn(id),
  consumer_group int references consumer_group(id),
  unique (transaction, consumer_group),

  last_updated timestamp default current_timestamp not null,
  created_at timestamp default current_timestamp not null
);

create table txn_offset_commit (
  id int generated always as identity primary key,

  transaction int references txn(id),
  consumer_group int references consumer_group(id),
  unique (transaction, consumer_group),

  producer_id bigint,

  generation_id int,
  member_id text,

  last_updated timestamp default current_timestamp not null,
  created_at timestamp default current_timestamp not null
);

create table txn_offset_commit_tp (
  id int generated always as identity primary key,

  offset_commit int references txn_offset_commit(id),
  topition int references topition(id),
  unique (offset_commit, topition),

  committed_offset bigint,
  leader_epoch int,
  metadata text,
  last_updated timestamp default current_timestamp not null,
  created_at timestamp default current_timestamp not null
);


commit;
