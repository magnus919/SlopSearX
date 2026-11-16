-- T04 exact-record CAS. All reads, type/collision/expiry checks precede writes.
local expected=cjson.decode(ARGV[1])
local replacement=cjson.decode(ARGV[2])
local expires=cjson.decode(ARGV[3])
local authorize=cjson.decode(ARGV[4])
local time=redis.call('TIME')
local now=tonumber(time[1])*1000+math.floor(tonumber(time[2])/1000)
for i,key in ipairs(KEYS) do
  local value=redis.pcall('GET',key)
  if type(value)=='table' then return -3 end
  -- Empty expected bytes mean absence; an extant empty string is a collision.
  if expected[i]=='' then
    if value~=false then return -1 end
  elseif value~=expected[i] then return -1 end
  if authorize[i]>0 and authorize[i]<=now then return -2 end
  if replacement[i]~='' and expires[i]<=now then return -2 end
end
for i,key in ipairs(KEYS) do
  if replacement[i]=='' then redis.call('DEL',key)
  else redis.call('SET',key,replacement[i],'PXAT',expires[i]) end
end
return 1
