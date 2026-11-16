-- Known validation failures return before the first mutation. A transport
-- failure after dispatch remains completion-unknown; callers never retry it.
local function kind(key)
  return redis.call('TYPE', key).ok
end
local function decode(raw)
  local ok, value = pcall(cjson.decode, raw)
  if not ok or type(value) ~= 'table' then return nil end
  return value
end
local function finite(value)
  return type(value) == 'number' and value == value and value ~= math.huge and value ~= -math.huge
end
local function corrupt()
  return cjson.encode({status='corrupt'})
end
local idem_kind = kind(KEYS[4])
if idem_kind ~= 'none' and idem_kind ~= 'string' then return corrupt() end
local idem = idem_kind == 'string' and redis.call('GET', KEYS[4]) or nil
if idem then
  local prior = decode(idem)
  if not prior or not finite(prior.expires_at) or type(prior.digest) ~= 'string'
    or type(prior.receipt_id) ~= 'string' or string.len(prior.receipt_id) == 0
    or string.len(prior.receipt_id) > 1024 then return corrupt() end
  if prior.expires_at > tonumber(ARGV[1]) then
    if prior.digest ~= ARGV[2] then return cjson.encode({status='conflict'}) end
    local prior_key = ARGV[3] .. prior.receipt_id
    local prior_kind = kind(prior_key)
    if prior_kind ~= 'none' and prior_kind ~= 'string' then return corrupt() end
    local existing = prior_kind == 'string' and redis.call('GET', prior_key) or nil
    if existing then
      local receipt = decode(existing)
      if not receipt or receipt.receipt_id ~= prior.receipt_id then return corrupt() end
      return cjson.encode({status='replayed', receipt=receipt})
    end
  end
end
local horizon_kind, index_kind, receipt_kind = kind(KEYS[1]), kind(KEYS[2]), kind(KEYS[3])
if (horizon_kind ~= 'none' and horizon_kind ~= 'string')
  or (index_kind ~= 'none' and index_kind ~= 'zset')
  or (receipt_kind ~= 'none' and receipt_kind ~= 'string') then return corrupt() end
-- An existing object or index member is an ID collision even if the logical
-- horizon expired. Never overwrite it or silently return another observation.
if receipt_kind ~= 'none' or redis.call('ZSCORE', KEYS[2], ARGV[9]) then
  return cjson.encode({status='collision'})
end
local horizon = horizon_kind == 'string' and decode(redis.call('GET', KEYS[1])) or nil
if horizon_kind == 'string' and (not horizon or not finite(horizon.expires_at)) then return corrupt() end
local expires_at = horizon and horizon.expires_at or nil
local new_horizon = not expires_at or expires_at <= tonumber(ARGV[1])
if new_horizon then expires_at = tonumber(ARGV[1]) + tonumber(ARGV[4]) end
local count = redis.call('ZCARD', KEYS[2])
if count > tonumber(ARGV[6]) then return corrupt() end
if not new_horizon and count >= tonumber(ARGV[6]) then return cjson.encode({status='capacity'}) end
local receipt = decode(ARGV[7])
if not receipt or not finite(expires_at) or type(receipt.receipt_id) ~= 'string'
  or string.len(receipt.receipt_id) == 0 or receipt.receipt_id ~= ARGV[9] then return corrupt() end
receipt.expires_at = expires_at
local ok, encoded = pcall(cjson.encode, receipt)
if not ok then return corrupt() end
if string.len(encoded) > tonumber(ARGV[10]) then return cjson.encode({status='size'}) end
local old = {}
if new_horizon then
  old = redis.call('ZRANGE', KEYS[2], 0, -1)
  for _, receipt_id in ipairs(old) do
    if string.len(receipt_id) == 0 or string.len(receipt_id) > 1024 then return corrupt() end
    local old_kind = kind(ARGV[3] .. receipt_id)
    if old_kind ~= 'none' and old_kind ~= 'string' then return corrupt() end
  end
end
local ttl = math.max(1, math.floor(expires_at - tonumber(ARGV[1])) + tonumber(ARGV[5]))
local horizon_encoded = cjson.encode({expires_at=expires_at})
local idempotency = cjson.encode({digest=ARGV[2], receipt_id=ARGV[9], expires_at=expires_at})
local response = cjson.encode({status='created', receipt=receipt})
-- Everything that can be checked without mutation has now been checked.
if idem then redis.call('DEL', KEYS[4]) end
if new_horizon then
  for _, receipt_id in ipairs(old) do redis.call('DEL', ARGV[3] .. receipt_id) end
  redis.call('DEL', KEYS[2])
  redis.call('SETEX', KEYS[1], tonumber(ARGV[4]) + tonumber(ARGV[5]), horizon_encoded)
end
redis.call('SETEX', KEYS[3], ttl, encoded)
redis.call('ZADD', KEYS[2], ARGV[8], ARGV[9])
redis.call('EXPIRE', KEYS[2], ttl)
redis.call('SETEX', KEYS[4], ttl, idempotency)
return response
