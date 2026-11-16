-- F10: TIME, INCRBY and expiry form one indivisible server operation.
-- All user arguments are validated before writes, including exact Lua ranges.
local window = tonumber(ARGV[1])
local rate = tonumber(ARGV[2])
local cost = tonumber(ARGV[3])
if not window or window < 1 or window > 4503599627370495 or window % 1 ~= 0
  or not rate or rate < 0 or rate > 9007199254740991
  or not cost or cost < 0 or cost > 9007199254740991 or cost % 1 ~= 0 then
  return redis.error_reply('invalid F10 quota arguments')
end
local t = redis.call('TIME')
local now = tonumber(t[1]) * 1000 + math.floor(tonumber(t[2]) / 1000)
local bucket = math.floor(now / window)
local key = KEYS[1] .. string.format('%.0f', bucket)
local count = redis.call('INCRBY', key, ARGV[3])
local ttl = redis.call('PTTL', key)
if ttl < 0 or count == cost then
  redis.call('PEXPIRE', key, string.format('%.0f', window * 2))
end
return {count <= rate and 1 or 0, redis.call('GET', key), key,
        string.format('%.0f', now), redis.call('PTTL', key)}
