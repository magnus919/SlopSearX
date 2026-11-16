local kind = redis.call('TYPE', KEYS[1]).ok
if kind ~= 'none' and kind ~= 'zset' then return -4 end
local instant = redis.call('TIME')
local now = tonumber(instant[1]) + tonumber(instant[2]) / 1000000
redis.call('ZREMRANGEBYSCORE', KEYS[1], '-inf', string.format('%.17g', now))
return 1
