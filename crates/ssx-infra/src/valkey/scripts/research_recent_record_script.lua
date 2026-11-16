
redis.call('ZADD', KEYS[1], ARGV[1], ARGV[2])
local count = redis.call('ZCARD', KEYS[1])
if count > tonumber(ARGV[3]) then
  redis.call('ZREMRANGEBYRANK', KEYS[1], 0, count - tonumber(ARGV[3]) - 1)
end
redis.call('EXPIRE', KEYS[1], ARGV[4])
return 1
