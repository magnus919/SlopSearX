
local raw = redis.call('GET', KEYS[1])
if not raw then redis.call('ZREM', KEYS[3], KEYS[1]); return {'missing',''} end
local current = cjson.decode(raw)
if current.state ~= 'queued' then redis.call('ZREM', KEYS[3], KEYS[1]); return {'not_queued',''} end
if tonumber(current.execution_deadline_at) <= tonumber(ARGV[1]) or
   tonumber(current.expires_at) <= tonumber(ARGV[1]) then
  current.state='failed'; current.stop_reason='deadline_expired'
  current.objectives.unmet={{objective='deadline_ms',reason='deadline_expired'}}
  local encoded=cjson.encode(current)
  redis.call('SETEX', KEYS[1], ARGV[3], encoded)
  redis.call('ZREM', KEYS[3], KEYS[1]); return {'deadline',encoded}
end
if redis.call('SET', KEYS[2], ARGV[2], 'NX', 'EX', ARGV[4]) == false then return {'busy',''} end
local stage = current.stages[tonumber(current.next_stage) + 1]
local cost = #stage.scope.selected_engines
if tonumber(current.budget.reserved) + cost > tonumber(current.budget.limit) then
  current.state='failed'; current.stop_reason='budget_exhausted'
  current.objectives.unmet={{objective='max_engine_calls',reason='budget_exhausted'}}
  local encoded=cjson.encode(current)
  redis.call('SETEX', KEYS[1], ARGV[3], encoded)
  redis.call('DEL', KEYS[2]); redis.call('ZREM', KEYS[3], KEYS[1]); return {'budget',encoded}
end
current.state='running'; current.owner_token=ARGV[2]
current.budget.reserved=tonumber(current.budget.reserved)+cost
stage.state='running'
local attempt={attempt_id=ARGV[5],started_at=tonumber(ARGV[6]),finished_at=cjson.null,
  state='running',reserved_engine_calls=cost,observed_engine_calls=cjson.null,cached=false,
  query_id=cjson.null,cursor=cjson.null,result_count=0,engine_outcomes={},
  enforcement=stage.enforcement,error=cjson.null,deadline_exceeded=false}
stage.attempts[#stage.attempts+1]=attempt
redis.call('SETEX', KEYS[1], ARGV[3], cjson.encode(current))
redis.call('ZREM', KEYS[3], KEYS[1])
redis.call('ZADD', KEYS[4], tonumber(ARGV[1])+tonumber(ARGV[4]), KEYS[1])
return {'claimed',cjson.encode(current)}
