-- Read bounded sorted-index candidates against one authoritative server instant.
local kind = redis.call('TYPE', KEYS[1]).ok
if kind ~= 'none' and kind ~= 'zset' then return {-4, {}} end
local instant = redis.call('TIME')
local now = tonumber(instant[1]) + tonumber(instant[2]) / 1000000
if ARGV[1] == 'due' then
  return {1, redis.call('ZRANGEBYSCORE', KEYS[1], '-inf', string.format('%.17g', now), 'LIMIT', 0, ARGV[2])}
elseif ARGV[1] == 'definitions' then
  redis.call('ZREMRANGEBYSCORE', KEYS[1], '-inf', string.format('%.17g', now))
  return {1, redis.call('ZRANGEBYSCORE', KEYS[1], string.format('%.17g', now), '+inf', 'LIMIT', 0, ARGV[2])}
elseif ARGV[1] == 'recent' then
  return {1, redis.call('ZREVRANGE', KEYS[1], 0, 199, 'WITHSCORES')}
end
return {-4, {}}
