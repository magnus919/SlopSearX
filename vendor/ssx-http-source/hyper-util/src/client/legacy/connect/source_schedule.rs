//! Private source scheduler; included unchanged by isolated deterministic tests.
use futures_util::future::Either;
use std::{future::Future, net::SocketAddr, time::Duration};
// Preserve the selected AnyIO iteration exactly: the first observed IPv6 is
// inserted at zero; the first IPv4 encountered AFTER it is inserted at one.
// Do not regroup all remaining addresses by family.
pub(super) fn source_address_order(addrs: impl IntoIterator<Item = SocketAddr>) -> Vec<SocketAddr> {
    let (mut v6_found, mut v4_found) = (false, false);
    let mut ordered = Vec::new();
    for addr in addrs {
        if addr.is_ipv6() && !v6_found {
            v6_found = true;
            ordered.insert(0, addr);
        } else if addr.is_ipv4() && !v4_found && v6_found {
            v4_found = true;
            ordered.insert(1, addr);
        } else {
            ordered.push(addr);
        }
    }
    ordered
}

// Same existing socket constructor/connect future; only the opt-in scheduler
// changes. No tasks are spawned: all attempts are owned by this future and its
// FuturesUnordered, including ready-but-not-selected loser sockets.
pub(super) async fn source_connect_race<T, E, F, Fut>(
    addrs: Vec<SocketAddr>,
    mut dial: F,
) -> Result<T, Vec<E>>
where
    F: FnMut(SocketAddr) -> Fut,
    Fut: Future<Output = Result<T, E>>,
{
    use futures_util::stream::{FuturesUnordered, StreamExt};
    let mut addresses = addrs.into_iter().enumerate().peekable();
    let mut pending = FuturesUnordered::new();
    let mut errors = Vec::new();
    let mut latest = None;
    let mut deadline = tokio::time::Instant::now();
    let mut launch = true;
    let mut exhausted = false;
    loop {
        if launch && !exhausted {
            if let Some((index, addr)) = addresses.next() {
                let future = dial(addr);
                pending.push(async move { (index, future.await) });
                latest = Some(index);
                exhausted = addresses.peek().is_none();
                deadline = tokio::time::Instant::now() + Duration::from_millis(250);
            } else {
                exhausted = true;
            }
        }
        launch = false;
        if pending.is_empty() {
            if exhausted {
                return Err(errors);
            }
            launch = true;
            continue;
        }
        let result = if exhausted {
            pending.next().await
        } else {
            match futures_util::future::select(
                Box::pin(pending.next()),
                Box::pin(tokio::time::sleep_until(deadline)),
            )
            .await
            {
                Either::Left((result, _)) => result,
                Either::Right(((), _)) => {
                    launch = true;
                    continue;
                }
            }
        };
        match result {
            Some((_, Ok(stream))) => return Ok(stream),
            Some((index, Err(error))) => {
                errors.push(error);
                // An OLDER pending attempt failing must not shorten the current
                // attempt's own250ms wait; source waits only its latest Event.
                if latest == Some(index) {
                    launch = true;
                }
            }
            None => {
                if exhausted {
                    return Err(errors);
                }
                launch = true;
            }
        }
    }
}
