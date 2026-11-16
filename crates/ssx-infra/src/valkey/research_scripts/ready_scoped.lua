-- Same microsecond precision as the accepted F07 ready-refresh/take scripts.
-- TIME and selection share one atomic observation; no rounding admits future work.
local now = redis.call('TIME')
local due = tonumber(now[1]) + tonumber(now[2]) / 1000000
return redis.call('ZRANGEBYSCORE', KEYS[1], '-inf', due, 'LIMIT', 0, 16)
