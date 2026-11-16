-- KEYS v2_ack, legacy_ack, consumers, stream; ARGV cursor, consumer, retention, capacity, legacy_unambiguous.
if #KEYS ~= 4 or #ARGV ~= 5 or not type_ok(KEYS[1],'string') or not type_ok(KEYS[3],'zset') or not type_ok(KEYS[4],'stream') then return {-4,''} end
local retention,capacity = integer(ARGV[3],1,31536000),integer(ARGV[4],1,100000)
if not retention or not capacity or not cursor_valid(ARGV[1]) or #ARGV[2] < 1 or #ARGV[2] > 128 or (ARGV[5] ~= '0' and ARGV[5] ~= '1') then return {-4,''} end
local current = redis.call('GET',KEYS[1])
if not current and ARGV[5] == '1' then
  if not type_ok(KEYS[2],'string') then return {-4,''} end
  current = redis.call('GET',KEYS[2])
end
if current and not cursor_valid(current) then return {-4,''} end
local now = instant()
local ttl = retention+300
-- Ambiguous legacy state is neither attributed nor mutated. A retained explicit
-- acknowledgement establishes this tenant/consumer's separate native authority.
redis.call('ZREMRANGEBYSCORE',KEYS[3],'-inf',now)
redis.call('XTRIM',KEYS[4],'MINID',cutoff(now,retention))
local tail = redis.call('XREVRANGE',KEYS[4],'+','-','COUNT',1)
if #tail == 0 then return {-3,''} end
if cursor_cmp(ARGV[1],tail[1][1]) > 0 then return {-2,''} end
local head = redis.call('XRANGE',KEYS[4],'-','+','COUNT',1)
if cursor_cmp(ARGV[1],head[1][1]) < 0 then return {-6,''} end
local advanced = not current or cursor_cmp(ARGV[1],current) > 0
if not current and not redis.call('ZSCORE',KEYS[3],ARGV[2]) and redis.call('ZCARD',KEYS[3]) >= capacity then return {-1,''} end
local acknowledged = advanced and ARGV[1] or current
redis.call('SETEX',KEYS[1],ttl,acknowledged)
redis.call('ZADD',KEYS[3],now+ttl,ARGV[2])
redis.call('EXPIRE',KEYS[3],ttl)
return {advanced and 1 or 0,acknowledged}
