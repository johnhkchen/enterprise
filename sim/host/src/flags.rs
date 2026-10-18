//! The host's arguments, read by hand (no dependencies): the study's own flags, and the host's.

/// What the run was asked for.
#[derive(Clone, Debug, Default)]
pub struct Flags {
    pub days: u64,
    pub csv: Option<String>,
    pub seeds: u64,
    /// Print the day's line every this many days (0: let the host choose).
    pub every: u64,
    pub inject: Option<u64>,
    pub river: bool,
    pub river_bridge: bool,
    pub comforts: bool,
    pub fleet: bool,
    pub services: bool,
    pub metal: bool,
    pub trades: bool,
    pub master: bool,
    pub war: Option<(u64, u64, u64)>,
    pub freight: bool,
    pub ridge: bool,
    pub realms: bool,
    /// The host's stand-in for the core's gap: the settlements' lockers' wants as their people's uses (the core's
    /// wagons ship nothing a town wants only for its lockers).
    pub people_use: bool,
    /// `--stop-when TOWN.field<value,...`: (town or "any", field, op, value).
    pub stop_when: Vec<(String, String, String, f64)>,
    /// `--dial name=value`, set before the first day.
    pub dials: Vec<(String, f64)>,
}

pub const USAGE: &str = "enterprise-sim [DAYS] [--csv OUT] [--seeds N] [--every N] [--inject DAY] [--river | --river-bridge] \
[--comforts] [--fleet] [--services] [--metal] [--trades] [--master] [--war M,W,P] [--freight] [--ridge] [--realms] [--people-use] \
[--stop-when TOWN.field<value,...] [--dial name=value ...]\n       enterprise-sim --sweep N [DAYS]";

impl Flags {
    pub fn parse(args: &[String]) -> Result<Flags, String> {
        let mut f = Flags { days: 10_000, seeds: 1, ..Default::default() };
        let mut i = 0;
        let value = |i: &mut usize, name: &str| -> Result<String, String> {
            *i += 1;
            args.get(*i).cloned().ok_or_else(|| format!("{name} wants a value"))
        };
        let number = |s: &str, name: &str| -> Result<u64, String> { s.parse().map_err(|_| format!("{name}: can't read {s:?}")) };
        while i < args.len() {
            let a = args[i].as_str();
            match a {
                "--csv" => f.csv = Some(value(&mut i, a)?),
                "--seeds" => f.seeds = number(&value(&mut i, a)?, a)?.max(1),
                "--every" => f.every = number(&value(&mut i, a)?, a)?,
                "--inject" => f.inject = Some(number(&value(&mut i, a)?, a)?),
                "--river" => f.river = true,
                "--river-bridge" => f.river_bridge = true,
                "--comforts" => f.comforts = true,
                "--fleet" => f.fleet = true,
                "--services" => f.services = true,
                "--metal" => f.metal = true,
                "--trades" => f.trades = true,
                "--master" => f.master = true,
                "--freight" => f.freight = true,
                "--ridge" => f.ridge = true,
                "--realms" => f.realms = true,
                "--people-use" => f.people_use = true,
                "--war" => {
                    let v = value(&mut i, a)?;
                    let d: Vec<u64> = v.split(',').map(|x| number(x, a)).collect::<Result<_, _>>()?;
                    if d.len() != 3 {
                        return Err(format!("--war wants M,W,P (mobilise, declare, peace), not {v:?}"));
                    }
                    f.war = Some((d[0], d[1], d[2]));
                }
                "--stop-when" => {
                    for cond in value(&mut i, a)?.split(',').filter(|c| !c.trim().is_empty()) {
                        f.stop_when.push(stop_condition(cond.trim())?);
                    }
                }
                "--dial" => {
                    let v = value(&mut i, a)?;
                    let (name, x) = v.split_once('=').ok_or_else(|| format!("--dial wants name=value, not {v:?}"))?;
                    let x: f64 = x.parse().map_err(|_| format!("--dial {name}: can't read {x:?}"))?;
                    f.dials.push((name.to_string(), x));
                }
                "-h" | "--help" => return Err(USAGE.to_string()),
                _ if !a.starts_with("--") && i == 0 => f.days = number(a, "DAYS")?,
                _ => return Err(format!("unknown argument {a:?}\n{USAGE}")),
            }
            i += 1;
        }
        if f.river && f.river_bridge {
            return Err("--river and --river-bridge: one river or the other".into());
        }
        Ok(f)
    }
}

/// `TOWN.field<value` (`<`, `<=`, `>`, `>=`), as the study reads it.
fn stop_condition(cond: &str) -> Result<(String, String, String, f64), String> {
    let at = cond.find(['<', '>']).ok_or_else(|| format!("--stop-when: can't read {cond:?} (TOWN.field<value)"))?;
    let (left, right) = cond.split_at(at);
    let (op, v) = if right.len() > 1 && right.as_bytes()[1] == b'=' { right.split_at(2) } else { right.split_at(1) };
    let (town, field) = left.split_once('.').ok_or_else(|| format!("--stop-when: can't read {cond:?} (TOWN.field<value)"))?;
    let v: f64 = v.trim().parse().map_err(|_| format!("--stop-when: can't read {cond:?}"))?;
    Ok((town.trim().to_string(), field.trim().to_string(), op.to_string(), v))
}
