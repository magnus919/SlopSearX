
local existing = redis.call('GET', KEYS[1])
if existing then
    local ok, record = pcall(cjson.decode, existing)
    if ok and type(record) == 'table' and record.job_id then
        return {0, record.job_id}
    end
    return {-1, ''}
end
redis.call('SETEX', KEYS[2], ARGV[1], ARGV[2])
redis.call('SETEX', KEYS[1], ARGV[1], ARGV[3])
return {1, ARGV[4]}
