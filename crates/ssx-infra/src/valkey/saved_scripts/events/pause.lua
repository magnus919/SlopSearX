-- KEYS definition,due,lease,baseline,recent,stream.
-- ARGV expected,next,expiry,created,next_due,id,event,retention,capacity.
local expected = {'string','zset','string','string','zset','stream'}
if #KEYS ~= 6 or #ARGV ~= 9 then return {-4,''} end
for i,key in ipairs(KEYS) do if not type_ok(key,expected[i]) then return {-4,''} end end
local raw = redis.call('GET',KEYS[1])
if not raw then return {-1,''} end
if raw ~= ARGV[1] then return {-2,''} end
local now = instant()
local expiry,created,due = tonumber(ARGV[3]),tonumber(ARGV[4]),tonumber(ARGV[5])
local retention,capacity = integer(ARGV[8],1,31536000),integer(ARGV[9],1,100000)
if not expiry or expiry ~= expiry or expiry > 9000000000000000-300 or expiry <= now or not created or created > now or not due or due ~= due or due == math.huge or due == -math.huge or not retention or not capacity or #ARGV[2] > 4194304 or #ARGV[7] == 0 or #ARGV[7] > 65536 or not stream_writable(KEYS[6]) then return {-4,''} end
local ttl = math.max(1,math.floor(expiry-now)+300)
redis.call('XTRIM',KEYS[6],'MINID',cutoff(now,retention))
if redis.call('XLEN',KEYS[6]) >= capacity then return {-5,''} end
local cursor = redis.call('XADD',KEYS[6],'*','payload',ARGV[7])
redis.call('EXPIRE',KEYS[6],retention+300)
redis.call('SETEX',KEYS[1],ttl,ARGV[2])
redis.call('ZREM',KEYS[2],ARGV[6])
redis.call('DEL',KEYS[3],KEYS[4])
redis.call('ZADD',KEYS[5],created,ARGV[6])
local excess = redis.call('ZCARD',KEYS[5])-200
if excess > 0 then redis.call('ZREMRANGEBYRANK',KEYS[5],0,excess-1) end
if redis.call('TTL',KEYS[5]) < ttl then redis.call('EXPIRE',KEYS[5],ttl) end
return {1,cursor}
