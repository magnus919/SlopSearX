-- KEYS stream, v2_ack, legacy_ack; ARGV requested-or-empty, limit, retention, legacy_unambiguous.
local function reject(status) return {status,'','','',{},'0','0'} end
if #KEYS ~= 3 or #ARGV ~= 4 or not type_ok(KEYS[1],'stream') or not type_ok(KEYS[2],'string') then return reject(-4) end
local limit,retention = integer(ARGV[2],1,100),integer(ARGV[3],1,31536000)
if not limit or not retention or (ARGV[4] ~= '0' and ARGV[4] ~= '1') then return reject(-4) end
local ack = redis.call('GET',KEYS[2])
local recovery = '0'
if not ack then
  if not type_ok(KEYS[3],'string') then return reject(-4) end
  local legacy = redis.call('GET',KEYS[3])
  if legacy and ARGV[4] == '0' then
    if ARGV[1] == '' then return reject(-7) end
    recovery = '1'
  elseif legacy then ack = legacy end
end
ack = ack or '0-0'
local requested = ARGV[1] == '' and ack or ARGV[1]
if not cursor_valid(ack) or not cursor_valid(requested) then return reject(-4) end
local now = instant()
-- First mutation follows all key/argument/legacy-ownership validation.
redis.call('XTRIM',KEYS[1],'MINID',cutoff(now,retention))
local first = redis.call('XRANGE',KEYS[1],'-','+','COUNT',1)
local head = #first > 0 and first[1][1] or ''
local entries = {}
local tail = redis.call('XREVRANGE',KEYS[1],'+','-','COUNT',1)
if #tail > 0 and cursor_cmp(requested,tail[1][1]) < 0 then
  local ms,seq = string.match(requested,'^([0-9]+)%-([0-9]+)$')
  if decimal_cmp(seq,'18446744073709551615') > 0 then seq = '18446744073709551615' end
  entries = redis.call('XRANGE',KEYS[1],'(' .. ms .. '-' .. seq,'+','COUNT',limit)
end
return {1,requested,ack,head,entries,tostring(now),recovery}
