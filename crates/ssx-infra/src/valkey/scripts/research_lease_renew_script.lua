
if redis.call('GET', KEYS[1]) ~= ARGV[1] then
    return 0
end
redis.call('SETEX', KEYS[1], ARGV[2], ARGV[1])
return 1
