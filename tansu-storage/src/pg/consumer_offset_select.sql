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

-- prepare consumer_offset_select(text, text, text, integer) as
select co.committed_offset

from cluster c
join consumer_group cg on cg.cluster = c.id
join topic t on t.cluster = c.id
join topition tp on tp.topic = t.id
join consumer_offset co on co.consumer_group = cg.id and co.topition = tp.id

where c.name = $1
and cg.name = $2
and t.name = $3
and tp.partition = $4;
