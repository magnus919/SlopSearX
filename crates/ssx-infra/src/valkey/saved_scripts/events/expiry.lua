-- KEYS definition,marker,stream; ARGV raw,expiry,event,retention,capacity.
if #KEYS ~= 3 or #ARGV ~= 5 or not type_ok(KEYS[1],'string') or not type_ok(KEYS[2],'string') or not type_ok(KEYS[3],'stream') then return {-4,''} end
local raw = redis.call('GET',KEYS[1])
if not raw or raw ~= ARGV[1] or redis.call('EXISTS',KEYS[2]) == 1 then return {0,''} end
local retention,capacity = integer(ARGV[4],1,31536000),integer(ARGV[5],1,100000)
local expiry,now = tonumber(ARGV[2]),instant()
if not expiry or expiry ~= expiry or not retention or not capacity or #ARGV[3] == 0 or #ARGV[3] > 65536 or not stream_writable(KEYS[3]) then return {-4,''} end
if expiry > now then return {0,''} end
redis.call('XTRIM',KEYS[3],'MINID',cutoff(now,retention))
if redis.call('XLEN',KEYS[3]) >= capacity then return {-5,''} end
local cursor = redis.call('XADD',KEYS[3],'*','payload',ARGV[3])
redis.call('EXPIRE',KEYS[3],retention+300)
redis.call('SETEX',KEYS[2],retention+300,'1')
return {1,cursor}
