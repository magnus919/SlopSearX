//! Browser-specific authority; F07 owns connection budgets and unknown outcomes.
use super::{Failure, Valkey};
use redis::Value;
use serde::{Serialize, de::DeserializeOwned};
use ssx_domain::browser_identity::BrowserError;
use std::sync::Arc;
const MAX: usize = 65_536;
const READ: &str = r#"
local t=redis.call('TYPE',KEYS[1]).ok
if t=='none' then return {0} end
if t~='string' then return {2} end
if redis.call('STRLEN',KEYS[1])>65536 then return {2} end
return {1,redis.call('GET',KEYS[1])}
"#;
// Preflight every key/type/value/collision before writes. Exact raw bytes fence stale callbacks.
const CHANGE: &str = r#"
local mode=ARGV[1]
local t=redis.call('TIME');local now=tonumber(t[1])*1000+math.floor(tonumber(t[2])/1000)
for i=1,#KEYS do local k=redis.call('TYPE',KEYS[i]).ok;if k~='none' and k~='string' then return 2 end end
local old=redis.call('GET',KEYS[1])
if mode=='create' then
 if old then return 3 end
elseif mode=='replace' then
 if KEYS[1]==KEYS[2] or KEYS[1]==KEYS[3] or old then return 3 end
else
 if not old then return 4 end
 if old~=ARGV[2] then return 3 end
 local ok,r=pcall(cjson.decode,old);if not ok then return 2 end
 if tonumber(r.expires_ms or r.absolute_expires_ms or 0)<=now then return 5 end
 if r.idle_expires_ms and tonumber(r.idle_expires_ms)<=now then return 5 end
end
if mode=='delete' then redis.call('DEL',KEYS[1]);return 1 end
if mode=='consume' then redis.call('DEL',KEYS[1]);return 1 end
local expires=tonumber(ARGV[4]);if not expires or expires<=now then return 5 end
if #ARGV[3]>65536 then return 2 end
if mode=='rotate' then
 if KEYS[1]==KEYS[2] or redis.call('EXISTS',KEYS[2])==1 then return 3 end
 redis.call('SET',KEYS[2],ARGV[3],'PXAT',ARGV[4]);redis.call('DEL',KEYS[1]);return 1
end
redis.call('SET',KEYS[1],ARGV[3],'PXAT',ARGV[4])
if mode=='replace' then redis.call('DEL',KEYS[2],KEYS[3]) end
return 1
"#;
const RATE: &str = r#"
local t=redis.call('TIME');local now=tonumber(t[1]);local window=tonumber(ARGV[2]);local slot=math.floor(now/window)
local key=KEYS[1]..':'..string.format('%.0f',slot)
local typ=redis.call('TYPE',key).ok;if typ~='none' and typ~='string' then return -1 end
local old=redis.call('GET',key);if old and (not string.match(old,'^%d+$') or tonumber(old)>1000000000) then return -1 end
local n=redis.call('INCR',key);if n==1 then redis.call('EXPIRE',key,window+1) end
if n<=tonumber(ARGV[1]) then return 1 else return 0 end
"#;
const AUDIT: &str = r#"
local t=redis.call('TIME');local day=math.floor(tonumber(t[1])/86400);local key=KEYS[1]..':'..string.format('%.0f',day)
local typ=redis.call('TYPE',key).ok;if typ~='none' and typ~='stream' then return 0 end
redis.call('XADD',key,'MAXLEN','=',10000,'*','timestamp',t[1]..'.'..t[2],'event',ARGV[1]);redis.call('EXPIRE',key,7776000);return 1
"#;
#[derive(Clone)]
pub struct BrowserStore {
    pub(crate) valkey: Arc<Valkey>,
}
impl BrowserStore {
    pub fn new(valkey: Arc<Valkey>) -> Self {
        Self { valkey }
    }
    pub async fn now(&self) -> Result<u64, BrowserError> {
        self.valkey
            .quota_time()
            .await
            .map(|t| t.milliseconds())
            .map_err(map)
    }
    async fn eval(
        &self,
        script: &str,
        keys: &[&str],
        args: &[String],
        write: bool,
    ) -> Result<Value, BrowserError> {
        let mut c = redis::cmd("EVAL");
        c.arg(script).arg(keys.len()).arg(keys).arg(args);
        self.valkey.execute(c, write).await.map_err(map)
    }
    pub async fn read<T: DeserializeOwned>(&self, key: &str) -> Result<(T, String), BrowserError> {
        let value = self.eval(READ, &[key], &[], false).await?;
        let Value::Array(v) = value else {
            return Err(BrowserError::Malformed);
        };
        match v.as_slice() {
            [Value::Int(0)] => Err(BrowserError::Missing),
            [Value::Int(1), Value::BulkString(b)] if b.len() <= MAX => {
                let s = std::str::from_utf8(b).map_err(|_| BrowserError::Malformed)?;
                let parsed = crate::browser_identity::strict_json(s.as_bytes())?;
                let record = serde_json::from_value(parsed).map_err(|_| BrowserError::Malformed)?;
                Ok((record, s.to_owned()))
            }
            _ => Err(BrowserError::Malformed),
        }
    }
    pub async fn change<T: Serialize>(
        &self,
        mode: &str,
        keys: &[&str],
        expected: &str,
        record: &T,
        expires: u64,
    ) -> Result<(), BrowserError> {
        let count = match mode {
            "create" | "consume" | "refresh" | "delete" => 1,
            "rotate" => 2,
            "replace" => 3,
            _ => return Err(BrowserError::InvalidInput),
        };
        if keys.len() != count
            || keys.iter().any(|k| k.is_empty() || k.len() > 1024)
            || expected.len() > MAX
        {
            return Err(BrowserError::InvalidInput);
        }
        let bytes = serde_json::to_string(record).map_err(|_| BrowserError::InvalidInput)?;
        if bytes.len() > MAX {
            return Err(BrowserError::InvalidInput);
        }
        let r = self
            .eval(
                CHANGE,
                keys,
                &[mode.into(), expected.into(), bytes, expires.to_string()],
                true,
            )
            .await?;
        match r {
            Value::Int(1) => Ok(()),
            Value::Int(2) => Err(BrowserError::Malformed),
            Value::Int(3) => Err(BrowserError::Conflict),
            Value::Int(4) => Err(BrowserError::Missing),
            Value::Int(5) => Err(BrowserError::Expired),
            _ => Err(BrowserError::Malformed),
        }
    }
    pub async fn revoke(&self, keys: &[&str]) -> Result<(), BrowserError> {
        // No session creation can reuse a handle; deletion is idempotent even after natural expiry.
        let mut c = redis::cmd("DEL");
        c.arg(keys);
        self.valkey.execute(c, true).await.map(|_| ()).map_err(map)
    }
    pub async fn rate(&self, key: &str, limit: u64, window: u64) -> Result<bool, BrowserError> {
        match self
            .eval(RATE, &[key], &[limit.to_string(), window.to_string()], true)
            .await?
        {
            Value::Int(1) => Ok(true),
            Value::Int(0) => Ok(false),
            _ => Err(BrowserError::Malformed),
        }
    }
    pub async fn audit(&self, key: &str, event: &str) -> Result<(), BrowserError> {
        match self.eval(AUDIT, &[key], &[event.into()], true).await? {
            Value::Int(1) => Ok(()),
            _ => Err(BrowserError::Malformed),
        }
    }
}
fn map(e: Failure) -> BrowserError {
    match e {
        Failure::Closed => BrowserError::Closed,
        Failure::Unavailable => BrowserError::Unavailable,
        Failure::CompletionUnknown | Failure::ScriptErrorMayHaveWritten => {
            BrowserError::CompletionUnknown
        }
        Failure::Malformed => BrowserError::Malformed,
        Failure::InvalidInput => BrowserError::InvalidInput,
    }
}
