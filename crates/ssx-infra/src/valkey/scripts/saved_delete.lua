
local raw = redis.call('GET', KEYS[1])
if not raw then return -1 end
local current = cjson.decode(raw)
if tonumber(current.revision) ~= tonumber(ARGV[1]) then return tonumber(current.revision) end
redis.call('DEL', KEYS[1])
redis.call('ZREM', KEYS[2], ARGV[2])
redis.call('ZREM', KEYS[3], ARGV[2])
redis.call('SETEX', KEYS[4], ARGV[3], ARGV[1])
redis.call('DEL', KEYS[5])
local reports = redis.call('ZRANGE', KEYS[6], 0, -1)
for _, run_id in ipairs(reports) do redis.call('DEL', ARGV[4] .. run_id) end
redis.call('DEL', KEYS[6])
redis.call('DEL', KEYS[7])
redis.call('DEL', KEYS[8])
return 0
