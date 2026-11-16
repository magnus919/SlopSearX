local record_type = redis.call('TYPE', KEYS[1]).ok
local index_type = redis.call('TYPE', KEYS[2]).ok
if (record_type ~= 'none' and record_type ~= 'string') or (index_type ~= 'none' and index_type ~= 'zset') then return -4 end
if redis.call('GET', KEYS[1]) ~= ARGV[1] then return 0 end
local instant = redis.call('TIME')
local now = tonumber(instant[1]) + tonumber(instant[2]) / 1000000
if tonumber(ARGV[2]) <= now then return 0 end
redis.call('ZADD', KEYS[2], 'GT', ARGV[2], ARGV[3])
return 1
