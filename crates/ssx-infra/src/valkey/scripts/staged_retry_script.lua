
local raw=redis.call('GET', KEYS[1]); if not raw then return {'missing',''} end
local current=cjson.decode(raw)
if tonumber(current.expires_at) <= tonumber(ARGV[1]) then return {'expired',''} end
for _, key in ipairs(current.retry_keys) do if key == ARGV[2] then return {'replay',raw} end end
if current.state == 'queued' or current.state == 'running' then return {'busy',raw} end
if current.state ~= 'failed' and current.state ~= 'interrupted' then return {'none',raw} end
local target=nil
for i=#current.stages,1,-1 do
  if current.stages[i].state == 'failed' or current.stages[i].state == 'interrupted' then
    target=current.stages[i]; break
  end
end
if not target then return {'none',raw} end
local cost=#target.scope.selected_engines
if tonumber(current.budget.reserved)+cost > tonumber(current.budget.limit) then return {'budget',raw} end
if tonumber(current.execution_deadline_at) <= tonumber(ARGV[1]) then return {'deadline',raw} end
current.retry_keys[#current.retry_keys+1]=ARGV[2]
current.state='queued'; current.stop_reason=cjson.null; current.next_stage=target.index; current.owner_token=cjson.null
local encoded=cjson.encode(current)
redis.call('SETEX', KEYS[1], ARGV[3], encoded)
redis.call('ZADD', KEYS[2], ARGV[1], KEYS[1])
return {'queued',encoded}
