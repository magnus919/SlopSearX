local requested = math.ceil(tonumber(ARGV[1]))
local current = redis.call('TTL', KEYS[1])
if current < requested then
  redis.call('SET', KEYS[1], '1', 'EX', requested)
  return requested
end
return current
