
redis.call('XTRIM', KEYS[1], 'MINID', ARGV[1])
local first = redis.call('XRANGE', KEYS[1], '-', '+', 'COUNT', 1)
local first_cursor = ''
if #first > 0 then first_cursor = first[1][1] end
local entries = redis.call('XRANGE', KEYS[1], '(' .. ARGV[2], '+', 'COUNT', tonumber(ARGV[3]))
return {first_cursor, entries}
