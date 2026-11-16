
if redis.call('GET', KEYS[2]) ~= ARGV[1] then return {-2, ''} end
local raw = redis.call('GET', KEYS[1])
if not raw then return {-1, ''} end
local current = cjson.decode(raw)
if tonumber(current.revision) ~= tonumber(ARGV[2]) or current.paused or current.deleted or
   tonumber(current.expires_at) <= tonumber(ARGV[3]) or current.policy_fingerprint ~= ARGV[4] then return {-3, ''} end
local event_cursor = ''
if ARGV[17] ~= '' then
  redis.call('XTRIM', KEYS[7], 'MINID', ARGV[18])
  if redis.call('XLEN', KEYS[7]) >= tonumber(ARGV[19]) then return {-4, ''} end
  event_cursor = redis.call('XADD', KEYS[7], '*', 'payload', ARGV[17])
  redis.call('EXPIRE', KEYS[7], ARGV[20])
end
redis.call('SETEX', KEYS[3], ARGV[5], ARGV[6])
redis.call('ZADD', KEYS[4], ARGV[7], ARGV[8])
redis.call('EXPIRE', KEYS[4], ARGV[5])
while redis.call('ZCARD', KEYS[4]) > tonumber(ARGV[9]) do
  local oldest = redis.call('ZRANGE', KEYS[4], 0, 0)
  if #oldest == 0 then break end
  redis.call('ZREM', KEYS[4], oldest[1])
  redis.call('DEL', ARGV[10] .. oldest[1])
end
redis.call('SETEX', KEYS[1], ARGV[12], ARGV[11])
redis.call('ZADD', KEYS[5], ARGV[13], ARGV[14])
redis.call('EXPIRE', KEYS[5], ARGV[12])
if ARGV[15] ~= '' then redis.call('SETEX', KEYS[6], ARGV[16], ARGV[15]) end
redis.call('DEL', KEYS[2])
return {1, event_cursor}
