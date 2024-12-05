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

-- prepare watermark_select (text, text, integer) as

with stable as (

select

t.id as topic, tp.id as topition, min(txn_po.offset_start) as offset

from

cluster c
join topic t on t.cluster = c.id
join topition tp on tp.topic = t.id
join txn on txn.cluster = c.id
join txn_detail txn_d on txn_d.transaction = txn.id
join txn_topition txn_tp on txn_tp.txn_detail = txn_d.id and txn_tp.topition = tp.id
join txn_produce_offset txn_po on txn_po.txn_topition = txn_tp.id

where

c.name = $1
and t.name = $2
and tp.partition = $3
and (txn_d.status = 'PREPARE_COMMIT' or txn_d.status = 'PREPARE_ABORT' or txn_d.status = 'BEGIN')

group by t.id, tp.id

)

select w.low, w.high, s.offset as stable

from

cluster c
join topic t on t.cluster = c.id
join topition tp on tp.topic = t.id
join watermark w on w.topition = tp.id
left join stable s on s.topic = t.id and s.topition = tp.id

where c.name = $1
and t.name = $2
and tp.partition = $3;
