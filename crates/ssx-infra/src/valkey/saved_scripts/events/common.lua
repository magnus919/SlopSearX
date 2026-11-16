local function type_ok(key, expected)
  local actual = redis.call('TYPE', key).ok
  return actual == 'none' or actual == expected
end
local function decimal(value)
  return value == '0' or (string.match(value, '^[1-9][0-9]*$') ~= nil)
end
local function cursor_valid(value)
  if #value > 64 then return false end
  local ms, seq = string.match(value, '^([0-9]+)%-([0-9]+)$')
  return ms and decimal(ms) and decimal(seq)
end
local function decimal_cmp(a,b)
  if #a ~= #b then return #a < #b and -1 or 1 end
  if a == b then return 0 end
  return a < b and -1 or 1
end
local function cursor_cmp(a,b)
  local am,as = string.match(a, '^([0-9]+)%-([0-9]+)$')
  local bm,bs = string.match(b, '^([0-9]+)%-([0-9]+)$')
  local cmp = decimal_cmp(am,bm)
  return cmp ~= 0 and cmp or decimal_cmp(as,bs)
end
local function integer(value, low, high)
  local n = tonumber(value)
  if not n or n % 1 ~= 0 or n < low or n > high then return nil end
  return n
end
local function instant()
  local t = redis.call('TIME')
  return tonumber(t[1]) + tonumber(t[2])/1000000
end
local function cutoff(now,retention)
  return string.format('%.0f',math.max(0,math.floor((now-retention)*1000))) .. '-0'
end
local function stream_writable(key)
  if redis.call('EXISTS', key) == 0 then return true end
  local info = redis.call('XINFO','STREAM',key)
  for i = 1,#info,2 do
    if info[i] == 'last-generated-id' then
      if info[i+1] == '18446744073709551615-18446744073709551615' then return false end
    end
  end
  return true
end
