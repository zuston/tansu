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

-- prepare cg_update (text, text, uuid, uuid, json) as
insert into consumer_group
(cluster, name, e_tag, detail)

select c.id, $2, $4, $5
from cluster c
where c.name = $1

on conflict (cluster, name)

do update set

detail = excluded.detail,
last_updated = excluded.last_updated,
e_tag = $4

where consumer_group.e_tag = $3

returning name, cluster, e_tag, detail;
