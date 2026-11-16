
local raw = redis.call('GET', KEYS[1])
local now = redis.call('TIME')
local due = tonumber(now[1]) + tonumber(now[2]) / 1000000
local active = false
if raw then
    local ok, job = pcall(cjson.decode, raw)
    active = ok and type(job) == "table" and job.tenant == ARGV[1] and
        (job.state == 'queued' or job.state == 'running')
end
if active then
    local ttl = redis.call('PTTL', KEYS[2])
    if ttl > 0 then due = due + ttl / 1000 end
    redis.call('ZADD', KEYS[3], due, ARGV[2])
    redis.call('ZADD', KEYS[4], 'NX', 0, ARGV[1])
else
    redis.call('ZREM', KEYS[3], ARGV[2])
    if redis.call('ZCARD', KEYS[3]) == 0 then
        redis.call('ZREM', KEYS[4], ARGV[1])
    end
end
return 1
