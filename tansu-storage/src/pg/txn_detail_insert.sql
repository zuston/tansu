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

insert into txn_detail (transaction, producer_epoch, transaction_timeout_ms)

select txn.id, pe.id, $5

from

cluster c
join txn on txn.cluster = c.id
join producer p on p.cluster = c.id
join producer_epoch pe on pe.producer = p.id

where

c.name = $1
and txn.name = $2
and p.id = $3
and pe.epoch = $4;
