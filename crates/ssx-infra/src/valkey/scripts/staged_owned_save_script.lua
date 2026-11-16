
if redis.call('GET', KEYS[2]) ~= ARGV[1] then return 0 end
redis.call('SETEX', KEYS[1], ARGV[2], ARGV[3])
redis.call('ZREM', KEYS[4], KEYS[1])
if ARGV[4] == 'queued' then redis.call('ZADD', KEYS[3], ARGV[5], KEYS[1]) end
redis.call('DEL', KEYS[2])
return 1
