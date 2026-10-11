//! HAR export from observed CDP timestamps. Missing measurements stay unknown.
use serde_json::{json, Value};
use std::collections::HashMap;
#[derive(Default)]
struct Hop {
    request: Value,
    response: Value,
    end: Option<f64>,
    finish: Value,
    data: Option<u64>,
    interception: Value,
    completion: String,
}
pub fn export(events: Vec<Value>, dropped: u64) -> Value {
    let mut hops = Vec::<Hop>::new();
    let mut active = HashMap::<String, usize>::new();
    for e in events {
        let p = &e["params"];
        let Some(id) = p["requestId"].as_str() else {
            continue;
        };
        match e["method"].as_str().unwrap_or("") {
            "Network.requestWillBeSent" => {
                if let Some(&i) = active.get(id) {
                    if p["redirectResponse"].is_object() {
                        hops[i].response = p["redirectResponse"].clone();
                        hops[i].end = p["timestamp"].as_f64();
                        hops[i].completion = "redirect".into();
                    }
                }
                active.insert(id.into(), hops.len());
                hops.push(Hop {
                    request: p.clone(),
                    ..Default::default()
                });
            }
            method => {
                if let Some(&i) = active.get(id) {
                    let h = &mut hops[i];
                    match method {
                        "Network.responseReceived" => h.response = p["response"].clone(),
                        "Network.loadingFinished" | "Network.loadingFailed" => {
                            h.end = p["timestamp"].as_f64();
                            h.finish = p.clone();
                            h.completion = method.into();
                        }
                        "Network.dataReceived" => {
                            if let Some(n) = p["dataLength"].as_u64() {
                                h.data = Some(h.data.unwrap_or(0).saturating_add(n));
                            }
                        }
                        "Delta.interception" => h.interception = p.clone(),
                        _ => {}
                    }
                }
            }
        }
    }
    let mut incomplete = 0;
    let mut entries = vec![];
    for h in hops {
        let p = &h.request;
        let req = &p["request"];
        let resp = &h.response;
        let (Some(start), Some(wall), Some(end)) =
            (p["timestamp"].as_f64(), p["wallTime"].as_f64(), h.end)
        else {
            incomplete += 1;
            continue;
        };
        if end < start {
            incomplete += 1;
            continue;
        }
        let t = &resp["timing"];
        let n = |key: &str| t[key].as_f64().filter(|n| *n >= 0.);
        let diff = |a: &str, b: &str| match (n(a), n(b)) {
            (Some(a), Some(b)) if b >= a => b - a,
            _ => -1.,
        };
        let headers_end = n("receiveHeadersEnd");
        let receive = match (t["requestTime"].as_f64(), headers_end) {
            (Some(r), Some(he)) if end * 1000. >= r * 1000. + he => end * 1000. - r * 1000. - he,
            _ => -1.,
        };
        let transfer = h.finish["encodedDataLength"]
            .as_i64()
            .or_else(|| resp["encodedDataLength"].as_i64())
            .unwrap_or(-1);
        let query = req["url"]
            .as_str()
            .and_then(|s| reqwest::Url::parse(s).ok())
            .map(|u| {
                u.query_pairs()
                    .map(|(k, v)| json!({"name":k,"value":v}))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let redirect = resp["headers"]
            .as_object()
            .and_then(|o| {
                o.iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case("location"))
                    .map(|(_, v)| v.clone())
            })
            .unwrap_or(json!(""));
        entries.push(json!({"startedDateTime":utc(wall),"time":(end-start)*1000.,"request":{"method":req["method"],"url":req["url"],"httpVersion":resp["protocol"].as_str().unwrap_or(""),"headers":headers(&req["headers"]),"cookies":[],"queryString":query,"headersSize":-1,"bodySize":-1},"response":{"status":resp["status"].as_u64().unwrap_or(0),"statusText":resp["statusText"].as_str().unwrap_or(""),"httpVersion":resp["protocol"].as_str().unwrap_or(""),"headers":headers(&resp["headers"]),"cookies":[],"content":{"size":h.data.map(|n|n as i64).unwrap_or(-1),"mimeType":resp["mimeType"].as_str().unwrap_or("")},"redirectURL":redirect,"headersSize":-1,"bodySize":-1},"cache":{},"timings":{"blocked":-1,"dns":diff("dnsStart","dnsEnd"),"connect":diff("connectStart","connectEnd"),"ssl":diff("sslStart","sslEnd"),"send":diff("sendStart","sendEnd"),"wait":diff("sendEnd","receiveHeadersEnd"),"receive":receive},"_requestId":p["requestId"],"_source":"CDP Network events","_completion":h.completion,"_encodedTransferBytes":transfer,"_error":h.finish["errorText"],"_interception":h.interception,"_fromDiskCache":resp["fromDiskCache"],"_fromServiceWorker":resp["fromServiceWorker"],"_timingNotes":"Unknown timings are -1, never estimated; wait ends at receiveHeadersEnd; receive is measured from headers end to loadingFinished; status 0 means no observed response"}));
    }
    json!({"log":{"version":"1.2","creator":{"name":"DeltaBrowser","version":env!("CARGO_PKG_VERSION")},"entries":entries,"_incompleteRequestsOmitted":incomplete,"_droppedCaptureEvents":dropped,"comment":"Observed CDP data only; redirect hops preserved. Bodies/cookies omitted; sensitive headers redacted. URLs and query parameters are NOT redacted. Unknown sizes/timings are -1. Decoded content size uses observed dataReceived only; encoded transfer bytes are separate (not misreported as body size). Bounded capture can omit events."}})
}
fn headers(v: &Value) -> Vec<Value> {
    v.as_object().map(|m|m.iter().map(|(k,val)|json!({"name":k,"value":if ["authorization","cookie","set-cookie","proxy-authorization","x-api-key"].contains(&k.to_ascii_lowercase().as_str()){"[REDACTED]".into()}else{val.as_str().unwrap_or("").to_owned()}})).collect()).unwrap_or_default()
}
pub fn utc(seconds: f64) -> String {
    let ms = (seconds * 1000.).floor() as i64;
    let days = ms.div_euclid(86400000);
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let y = y + i64::from(m <= 2);
    let s = ms.div_euclid(1000).rem_euclid(86400);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{:03}Z",
        s / 3600,
        s / 60 % 60,
        s % 60,
        ms.rem_euclid(1000)
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timestamps() {
        assert_eq!(utc(0.125), "1970-01-01T00:00:00.125Z");
        assert_eq!(utc(-0.001), "1969-12-31T23:59:59.999Z");
    }
    #[test]
    fn missing_not_fabricated() {
        let e = vec![
            json!({"method":"Network.requestWillBeSent","params":{"requestId":"1","timestamp":10.,"wallTime":100.,"request":{"url":"https://example.com/","method":"GET"}}}),
        ];
        assert_eq!(export(e, 0)["log"]["_incompleteRequestsOmitted"], 1);
    }
}
