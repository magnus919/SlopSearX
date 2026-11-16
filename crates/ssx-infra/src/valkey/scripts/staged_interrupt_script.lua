
if redis.call('EXISTS', KEYS[2]) == 1 then return 0 end
local raw=redis.call('GET', KEYS[1]); if not raw then redis.call('ZREM', KEYS[3], KEYS[1]); return 0 end
local current=cjson.decode(raw)
if current.state ~= 'running' then redis.call('ZREM', KEYS[3], KEYS[1]); return 0 end
local stage=current.stages[tonumber(current.next_stage)+1]
local attempt=stage.attempts[#stage.attempts]
attempt.state='interrupted'; attempt.finished_at=tonumber(ARGV[1]); attempt.observed_engine_calls=cjson.null
stage.state='interrupted'; current.state='interrupted'; current.stop_reason='interrupted'
current.owner_token=cjson.null; current.budget.observed=cjson.null
redis.call('SETEX', KEYS[1], ARGV[2], cjson.encode(current)); redis.call('ZREM', KEYS[3], KEYS[1]); return 1
