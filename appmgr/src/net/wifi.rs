use std::collections::HashMap;
use std::time::Duration;

use rpc_toolkit::command;
use tokio::process::Command;

use crate::context::EitherContext;
use crate::util::display_none;
use crate::{Error, ErrorKind};

#[command(subcommands(add, connect, delete, get))]
pub async fn wifi(#[context] ctx: EitherContext) -> Result<EitherContext, Error> {
    Ok(ctx)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AddWifiReq {
    ssid: String,
    password: String,
    country: String,
    priority: isize,
    connect: bool,
}
#[command(rpc_only)]
pub async fn add(#[context] _ctx: EitherContext, #[arg] req: AddWifiReq) -> Result<(), Error> {
    let wpa_supplicant = WpaCli { interface: "wlan0" };
    if !req.ssid.is_ascii() {
        return Err(Error::new(
            anyhow::anyhow!("SSID not in the ASCII charset"),
            ErrorKind::WifiError,
        ));
    }
    if !req.password.is_ascii() {
        return Err(Error::new(
            anyhow::anyhow!("Wifi Password not in the ASCII charset"),
            ErrorKind::WifiError,
        ));
    }
    async fn add_procedure<'a>(wpa_supplicant: WpaCli<'a>, req: &AddWifiReq) -> Result<(), Error> {
        log::info!("Adding new WiFi network: '{}'", req.ssid);
        wpa_supplicant
            .add_network(&req.ssid, &req.password, req.priority)
            .await?;
        if req.connect {
            let current = wpa_supplicant.get_current_network().await?;
            let connected = wpa_supplicant.select_network(&req.ssid).await?;
            if !connected {
                log::error!("Faild to add new WiFi network: '{}'", req.ssid);
                wpa_supplicant.remove_network(&req.ssid).await?;
                match current {
                    None => {}
                    Some(current) => {
                        wpa_supplicant.select_network(&current).await?;
                    }
                }
            }
        }
        Ok(())
    }
    tokio::spawn(async move {
        match add_procedure(wpa_supplicant, &req).await {
            Err(e) => {
                log::error!("Failed to add new Wifi network '{}': {}", req.ssid, e);
            }
            Ok(_) => {}
        }
    });
    Ok(())
}

#[command(display(display_none))]
pub async fn connect(#[context] _ctx: EitherContext, #[arg] ssid: String) -> Result<(), Error> {
    if !ssid.is_ascii() {
        return Err(Error::new(
            anyhow::anyhow!("SSID not in the ASCII charset"),
            ErrorKind::WifiError,
        ));
    }
    async fn connect_procedure<'a>(wpa_supplicant: WpaCli<'a>, ssid: &String) -> Result<(), Error> {
        let current = wpa_supplicant.get_current_network().await?;
        let connected = wpa_supplicant.select_network(&ssid).await?;
        if connected {
            log::info!("Successfully connected to WiFi: '{}'", ssid);
        } else {
            log::error!("Failed to connect to WiFi: '{}'", ssid);
            match current {
                None => {
                    log::error!("No WiFi to revert to!");
                }
                Some(current) => {
                    wpa_supplicant.select_network(&current).await?;
                }
            }
        }
        Ok(())
    }
    let wpa_supplicant = WpaCli { interface: "wlan0" };
    tokio::spawn(async move {
        match connect_procedure(wpa_supplicant, &ssid).await {
            Err(e) => {
                log::error!("Failed to connect to WiFi network '{}': {}", &ssid, e);
            }
            Ok(_) => {}
        }
    });
    Ok(())
}

#[command(display(display_none))]
pub async fn delete(#[context] _ctx: EitherContext, #[arg] ssid: String) -> Result<(), Error> {
    if !ssid.is_ascii() {
        return Err(Error::new(
            anyhow::anyhow!("SSID not in ASCII charset"),
            ErrorKind::WifiError,
        ));
    }
    let wpa_supplicant = WpaCli { interface: "wlan0" };
    let current = wpa_supplicant.get_current_network().await?;
    match current {
        None => {
            wpa_supplicant.remove_network(&ssid).await?;
        }
        Some(current) => {
            if current == ssid {
                if interface_connected("eth0").await? {
                    wpa_supplicant.remove_network(&ssid).await?;
                } else {
                    return Err(Error::new(anyhow::anyhow!("Forbidden: Deleting this Network would make your Embassy Unreachable. Either connect to ethernet or connect to a different WiFi network to remedy this."), ErrorKind::WifiError));
                }
            }
        }
    }
    Ok(())
}
#[derive(serde::Serialize, serde::Deserialize)]
pub struct WiFiInfo {
    ssids: Vec<String>,
    selected: Option<String>,
    connected: Option<String>,
    country: String,
    ethernet: bool,
    signal_strength: Option<usize>,
}
#[command(display(display_none))]
pub async fn get(#[context] _ctx: EitherContext) -> Result<WiFiInfo, Error> {
    let wpa_supplicant = WpaCli { interface: "wlan0" };
    let ssids_task = async {
        Result::<Vec<String>, Error>::Ok(
            wpa_supplicant
                .list_networks_low()
                .await?
                .into_keys()
                .collect::<Vec<String>>(),
        )
    };
    let current_task = wpa_supplicant.get_current_network();
    let country_task = wpa_supplicant.get_country_low();
    let ethernet_task = interface_connected("eth0");
    let rssi_task = wpa_supplicant.signal_poll_low();
    let (ssids_res, current_res, country_res, ethernet_res, rssi_res) = tokio::join!(
        ssids_task,
        current_task,
        country_task,
        ethernet_task,
        rssi_task
    );
    let current = current_res?;
    let signal_strength = match rssi_res? {
        None => None,
        Some(x) if x <= -100 => Some(0 as usize),
        Some(x) if x >= -50 => Some(100 as usize),
        Some(x) => Some(2 * (x + 100) as usize),
    };
    Ok(WiFiInfo {
        ssids: ssids_res?,
        selected: current.clone(),
        connected: current,
        country: country_res?,
        ethernet: ethernet_res?,
        signal_strength,
    })
}

pub struct WpaCli<'a> {
    interface: &'a str,
}
#[derive(Clone)]
pub struct NetworkId(String);
pub enum NetworkAttr {
    Ssid(String),
    Psk(String),
    Priority(isize),
    ScanSsid(bool),
}
impl std::fmt::Display for NetworkAttr {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        use NetworkAttr::*;
        match self {
            Ssid(s) => write!(f, "\"{}\"", s),
            Psk(s) => write!(f, "\"{}\"", s),
            Priority(n) => write!(f, "{}", n),
            ScanSsid(b) => {
                if *b {
                    write!(f, "1")
                } else {
                    write!(f, "0")
                }
            }
        }
    }
}
impl<'a> WpaCli<'a> {
    // Low Level
    pub async fn add_network_low(&self) -> Result<NetworkId, Error> {
        let r = Command::new("wpa_cli")
            .arg("-i")
            .arg(self.interface)
            .arg("add_network")
            .output()
            .await?;
        check_exit_status(r.status)?;
        let s = std::str::from_utf8(&r.stdout)?;
        Ok(NetworkId(s.trim().to_owned()))
    }
    pub async fn set_network_low(&self, id: &NetworkId, attr: &NetworkAttr) -> Result<(), Error> {
        let r = Command::new("wpa_cli")
            .arg("-i")
            .arg(self.interface)
            .arg("set_network")
            .arg(&id.0)
            .arg(format!("{}", attr))
            .output()
            .await?;
        check_exit_status(r.status)?;
        Ok(())
    }
    pub async fn set_country_low(&self, country_code: &str) -> Result<(), Error> {
        let r = Command::new("wpa_cli")
            .arg("-i")
            .arg(self.interface)
            .arg("set")
            .arg("country")
            .arg(country_code)
            .output()
            .await?;
        check_exit_status(r.status)?;
        Ok(())
    }
    pub async fn get_country_low(&self) -> Result<String, Error> {
        let r = Command::new("wpa_cli")
            .arg("-i")
            .arg(self.interface)
            .arg("get")
            .arg("country")
            .output()
            .await?;
        check_exit_status(r.status)?;
        Ok(String::from_utf8(r.stdout)?)
    }
    pub async fn enable_network_low(&self, id: &NetworkId) -> Result<(), Error> {
        let r = Command::new("wpa_cli")
            .arg("-i")
            .arg(self.interface)
            .arg("enable_network")
            .arg(&id.0)
            .output()
            .await?;
        check_exit_status(r.status)?;
        Ok(())
    }
    pub async fn save_config_low(&self) -> Result<(), Error> {
        let r = Command::new("wpa_cli")
            .arg("-i")
            .arg(self.interface)
            .arg("save_config")
            .output()
            .await?;
        check_exit_status(r.status)?;
        Ok(())
    }
    pub async fn remove_network_low(&self, id: NetworkId) -> Result<(), Error> {
        let r = Command::new("wpa_cli")
            .arg("-i")
            .arg(self.interface)
            .arg("remove_network")
            .arg(&id.0)
            .output()
            .await?;
        check_exit_status(r.status)?;
        Ok(())
    }
    pub async fn reconfigure_low(&self) -> Result<(), Error> {
        let r = Command::new("wpa_cli")
            .arg("-i")
            .arg(self.interface)
            .arg("reconfigure")
            .output()
            .await?;
        check_exit_status(r.status)?;
        Ok(())
    }
    pub async fn list_networks_low(&self) -> Result<HashMap<String, NetworkId>, Error> {
        let r = Command::new("wpa_cli")
            .arg("-i")
            .arg(self.interface)
            .arg("list_networks")
            .output()
            .await?;
        check_exit_status(r.status)?;
        Ok(String::from_utf8(r.stdout)?
            .lines()
            .skip(1)
            .filter_map(|l| {
                let mut cs = l.split("\t");
                let nid = NetworkId(cs.next()?.to_owned());
                let ssid = cs.next()?.to_owned();
                Some((ssid, nid))
            })
            .collect::<HashMap<String, NetworkId>>())
    }
    pub async fn select_network_low(&self, id: &NetworkId) -> Result<(), Error> {
        let r = Command::new("wpa_cli")
            .arg("-i")
            .arg(self.interface)
            .arg("select_network")
            .arg(&id.0)
            .output()
            .await?;
        check_exit_status(r.status)?;
        Ok(())
    }
    pub async fn new_password_low(&self, id: &NetworkId, pass: &str) -> Result<(), Error> {
        let r = Command::new("wpa_cli")
            .arg("-i")
            .arg(self.interface)
            .arg("new_password")
            .arg(&id.0)
            .arg(pass)
            .output()
            .await?;
        check_exit_status(r.status)?;
        Ok(())
    }
    pub async fn signal_poll_low(&self) -> Result<Option<isize>, Error> {
        let r = Command::new("wpa_cli")
            .arg("-i")
            .arg(self.interface)
            .arg("signal_poll")
            .output()
            .await?;
        check_exit_status(r.status)?;
        let e = || {
            Error::new(
                anyhow::anyhow!("Invalid output from wpa_cli signal_poll"),
                ErrorKind::WifiError,
            )
        };
        let output = String::from_utf8(r.stdout)?;
        Ok(if output == "FAIL" {
            None
        } else {
            let l = output.lines().next().ok_or_else(e)?;
            let rssi = l.split("=").nth(1).ok_or_else(e)?.parse()?;
            Some(rssi)
        })
    }

    // High Level
    pub async fn check_network(&self, ssid: &str) -> Result<Option<NetworkId>, Error> {
        Ok(self
            .list_networks_low()
            .await?
            .get(ssid)
            .map(|a| (*a).clone()))
    }
    pub async fn select_network(&self, ssid: &str) -> Result<bool, Error> {
        let m_id = self.check_network(ssid).await?;
        match m_id {
            None => Err(Error::new(
                anyhow::anyhow!("SSID Not Found"),
                ErrorKind::WifiError,
            )),
            Some(x) => {
                self.select_network_low(&x).await?;
                self.save_config_low().await?;
                let connect = async {
                    let mut current;
                    loop {
                        current = self.get_current_network().await;
                        match &current {
                            Ok(Some(_)) => {
                                break;
                            }
                            _ => {}
                        }
                    }
                    current
                };
                let timeout = tokio::time::sleep(Duration::from_secs(20));
                let res = tokio::select! {
                    net = connect => { net? }
                    _ = timeout => { None }
                };
                Ok(match res {
                    None => false,
                    Some(net) => net == ssid,
                })
            }
        }
    }
    pub async fn get_current_network(&self) -> Result<Option<String>, Error> {
        let r = Command::new("iwgetid")
            .arg(self.interface)
            .arg("--raw")
            .output()
            .await?;
        check_exit_status(r.status)?;
        let output = String::from_utf8(r.stdout)?;
        if output == "" {
            Ok(None)
        } else {
            Ok(Some(output))
        }
    }
    pub async fn remove_network(&self, ssid: &str) -> Result<bool, Error> {
        match self.check_network(ssid).await? {
            None => Ok(false),
            Some(x) => {
                self.remove_network_low(x).await?;
                self.save_config_low().await?;
                self.reconfigure_low().await?;
                Ok(true)
            }
        }
    }
    pub async fn add_network(&self, ssid: &str, psk: &str, priority: isize) -> Result<(), Error> {
        use NetworkAttr::*;
        let nid = match self.check_network(ssid).await? {
            None => {
                let nid = self.add_network_low().await?;
                self.set_network_low(&nid, &Ssid(ssid.to_owned())).await?;
                self.set_network_low(&nid, &Psk(psk.to_owned())).await?;
                self.set_network_low(&nid, &Priority(priority)).await?;
                self.set_network_low(&nid, &ScanSsid(true)).await?;
                Result::<NetworkId, Error>::Ok(nid)
            }
            Some(nid) => {
                self.new_password_low(&nid, psk).await?;
                Ok(nid)
            }
        }?;
        self.enable_network_low(&nid).await?;
        self.save_config_low().await?;
        Ok(())
    }
}

fn check_exit_status(status: std::process::ExitStatus) -> Result<(), Error> {
    status.code().map_or(
        Err(Error::new(
            anyhow::anyhow!("wpa_cli was signal terminated"),
            ErrorKind::WifiError,
        )),
        |ec| {
            if ec == 0 {
                Ok(())
            } else {
                Err(Error::new(
                    anyhow::anyhow!("wpa_cli exited with nonzero exit code {}", ec),
                    ErrorKind::WifiError,
                ))
            }
        },
    )
}

pub async fn interface_connected(interface: &str) -> Result<bool, Error> {
    let out = Command::new("ifconfig").arg(interface).output().await?;
    let v = std::str::from_utf8(&out.stdout)?
        .lines()
        .filter(|s| s.contains("inet"))
        .next();
    Ok(!v.is_none())
}

#[tokio::test]
pub async fn test_interface_connected() {
    println!("{}", interface_connected("wlp5s0").await.unwrap());
    println!("{}", interface_connected("enp4s0f1").await.unwrap());
}

#[tokio::test]
pub async fn test_signal_strength() {
    let wpa = WpaCli {
        interface: "wlp5s0",
    };
    println!("{}", wpa.signal_poll_low().await.unwrap().unwrap())
}
