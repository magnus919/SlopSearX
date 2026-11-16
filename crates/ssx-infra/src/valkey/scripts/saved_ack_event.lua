
redis.call('ZREMRANGEBYSCORE', KEYS[2], '-inf', ARGV[1])
redis.call('XTRIM', KEYS[3], 'MINID', ARGV[7])
local tail = redis.call('XREVRANGE', KEYS[3], '+', '-', 'COUNT', 1)
if #tail == 0 then return -3 end
local function parts(value)
  local dash = string.find(value, '-')
  return tonumber(string.sub(value, 1, dash - 1)), tonumber(string.sub(value, dash + 1))
end
local requested_ms, requested_seq = parts(ARGV[2])
local tail_ms, tail_seq = parts(tail[1][1])
if requested_ms > tail_ms or (requested_ms == tail_ms and requested_seq > tail_seq) then return -2 end
local head = redis.call('XRANGE', KEYS[3], '-', '+', 'COUNT', 1)
local head_ms, head_seq = parts(head[1][1])
if requested_ms < head_ms or (requested_ms == head_ms and requested_seq < head_seq) then return -4 end
local current = redis.call('GET', KEYS[1])
if current then
  local current_ms, current_seq = parts(current)
  if requested_ms < current_ms or (requested_ms == current_ms and requested_seq <= current_seq) then
    redis.call('EXPIRE', KEYS[1], ARGV[3])
    redis.call('ZADD', KEYS[2], ARGV[4], ARGV[5])
    redis.call('EXPIRE', KEYS[2], ARGV[3])
    return 0
  end
elseif redis.call('ZCARD', KEYS[2]) >= tonumber(ARGV[6]) then
  return -1
end
redis.call('SETEX', KEYS[1], ARGV[3], ARGV[2])
redis.call('ZADD', KEYS[2], ARGV[4], ARGV[5])
redis.call('EXPIRE', KEYS[2], ARGV[3])
return 1
