
local now = redis.call('TIME')
local due = tonumber(now[1]) + tonumber(now[2]) / 1000000
for i = 1, 16 do
    local tenants = redis.call('ZRANGE', KEYS[1], 0, 0)
    if #tenants == 0 then return {} end
    local tenant = tenants[1]
    local ready = ARGV[1] .. ':tenant:' .. tenant
    local jobs = redis.call('ZRANGEBYSCORE', ready, '-inf', due, 'LIMIT', 0, 1)
    if redis.call('ZCARD', ready) == 0 then
        redis.call('ZREM', KEYS[1], tenant)
    else
        redis.call('ZADD', KEYS[1], redis.call('INCR', KEYS[2]), tenant)
    end
    if #jobs > 0 then
        -- Reservation is non-destructive: worker death before lease acquisition
        -- makes this candidate visible again without waiting for reconciliation.
        redis.call('ZADD', ready, due + 5, jobs[1])
        return {tenant, jobs[1]}
    end
end
return {}
