-- Never remove a recreated or concurrently updated definition's index hint.
local record_type = redis.call('TYPE', KEYS[1]).ok
local index_type = redis.call('TYPE', KEYS[2]).ok
if (record_type ~= 'none' and record_type ~= 'string') or (index_type ~= 'none' and index_type ~= 'zset') then return -4 end
local raw = redis.call('GET', KEYS[1])
if ARGV[1] == 'missing' then
  if raw ~= false then return 0 end
elseif raw ~= ARGV[2] then return 0 end
redis.call('ZREM', KEYS[2], ARGV[3])
return 1
