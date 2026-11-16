if redis.call('GET',KEYS[1]) ~= ARGV[7] then return {'not_queued',''} end
