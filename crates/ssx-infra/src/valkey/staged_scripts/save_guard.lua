if redis.call('GET',KEYS[1]) ~= ARGV[6] then return 0 end
