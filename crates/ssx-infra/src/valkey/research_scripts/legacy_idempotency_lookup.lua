-- A legacy alias has authority only with a live, exactly bound source record.
local raw = redis.pcall('GET', KEYS[1])
if type(raw) ~= 'string' then return false end
local ok, mapping = pcall(cjson.decode, raw)
if not ok or type(mapping) ~= 'table' or type(mapping.job_id) ~= 'string' or mapping.job_id == '' or string.find(mapping.job_id, ':', 1, true) then return false end
local key = ARGV[1] .. mapping.job_id
local record = redis.pcall('GET', key)
if type(record) ~= 'string' then return false end
local good, job = pcall(cjson.decode, record)
if not good or type(job) ~= 'table' or job.job_id ~= mapping.job_id or job.tenant ~= ARGV[2] or job.idempotency_key ~= ARGV[3] then return false end
if redis.call('PTTL', KEYS[1]) <= 0 or redis.call('PTTL', key) <= 0 then return false end
return mapping.job_id
