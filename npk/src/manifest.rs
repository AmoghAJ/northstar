// Copyright (c) 2019 - 2020 ESRLabs
//
//   Licensed under the Apache License, Version 2.0 (the "License");
//   you may not use this file except in compliance with the License.
//   You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
//   Unless required by applicable law or agreed to in writing, software
//   distributed under the License is distributed on an "AS IS" BASIS,
//   WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//   See the License for the specific language governing permissions and
//   limitations under the License.

use lazy_static::lazy_static;
use serde::{
    de::{Deserializer, Visitor},
    ser::{SerializeMap, Serializer},
    Deserialize, Serialize,
};
use std::{
    collections::{HashMap, HashSet},
    fmt, io,
    path::PathBuf,
    str::FromStr,
};
use thiserror::Error;

/// A container version. Versions follow the semver format
#[derive(Clone, PartialOrd, Hash, Eq, PartialEq)]
pub struct Version(pub semver::Version);

pub type Name = String;

impl Version {
    #[allow(dead_code)]
    pub fn parse(s: &str) -> Result<Version, semver::SemVerError> {
        Ok(Version(semver::Version::parse(s)?))
    }
}

impl Default for Version {
    fn default() -> Version {
        Version(semver::Version::new(0, 0, 0))
    }
}

/// Serde serialization for `Version`
impl Serialize for Version {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0.to_string())
    }
}

/// Serde deserialization for `Version`
impl<'de> Deserialize<'de> for Version {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct VersionVisitor;

        impl<'de> Visitor<'de> for VersionVisitor {
            type Value = Version;
            fn visit_str<E>(self, str_data: &str) -> Result<Version, E>
            where
                E: serde::de::Error,
            {
                semver::Version::parse(str_data).map(Version).map_err(|_| {
                    serde::de::Error::invalid_value(::serde::de::Unexpected::Str(str_data), &self)
                })
            }

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> ::std::fmt::Result {
                formatter.write_str("string v0.0.0")
            }
        }

        deserializer.deserialize_str(VersionVisitor)
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Debug for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum OnExit {
    /// Container is restarted n number and not started anymore after n exits
    #[serde(rename = "restart")]
    Restart(u32),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CGroupMem {
    /// Limit im bytes
    pub limit: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CGroupCpu {
    /// CPU shares assigned to this container. CGroups cpu divide
    /// the ressource CPU into 1024 shares
    pub shares: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CGroups {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mem: Option<CGroupMem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu: Option<CGroupCpu>,
}

#[derive(Clone, Eq, PartialEq, Debug, Hash, Serialize, Deserialize)]
pub enum MountFlag {
    /// Bind mount
    #[serde(rename = "rw")]
    Rw,
    // Mount noexec
    // #[serde(rename = "noexec")]
    // NoExec,
    // Mount noexec
    // #[serde(rename = "nosuid")]
    // NoSuid,
}

/// Configuration for the /dev mount
#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub enum Dev {
    /// Bind mount the full /dev of the host
    #[serde(rename = "full")]
    Full,
}

/// Mounts
#[derive(Clone, Eq, PartialEq, Debug)]
pub enum Mount {
    /// Mount a directory from a resouce
    Resource {
        name: String,
        version: Version,
        dir: PathBuf,
    },
    /// Bind mount of a host dir with flags
    Bind {
        host: PathBuf,
        flags: HashSet<MountFlag>,
    },
    /// Mount /dev with flavor `dev`
    Dev { r#type: Dev },
    /// Mount a rw host directory dedicated to this container rw
    Persist,
    /// Mount a tmpfs with size
    Tmpfs { size: u64 },
}

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct Manifest {
    /// Name of container
    pub name: Name,
    /// Container version
    pub version: Version,
    /// Path to init
    #[serde(skip_serializing_if = "Option::is_none")]
    pub init: Option<PathBuf>,
    /// Additional arguments for the application invocation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
    /// Environment passed to container
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,
    /// Autostart this container upon north startup
    #[serde(skip_serializing_if = "Option::is_none")]
    pub autostart: Option<bool>,
    /// Action on application exit
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_exit: Option<OnExit>,
    /// CGroup config
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cgroups: Option<CGroups>,
    /// Seccomp configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seccomp: Option<HashMap<String, String>>,
    /// Number of instances to mount of this container
    /// The name get's extended with the instance id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instances: Option<u32>,
    /// List of bind mounts and resources
    #[serde(
        with = "MountsSerialization",
        skip_serializing_if = "HashMap::is_empty"
    )]
    #[serde(default)]
    pub mounts: HashMap<PathBuf, Mount>,
}

/// Configuration for the persist mounts
#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub enum Persist {
    #[serde(rename = "persist")]
    Persist,
}

struct MountsSerialization;

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum MountSource {
    Resource {
        resource: String,
    },
    Tmpfs {
        #[serde(deserialize_with = "deserialize_tmpfs")]
        tmpfs: u64,
    },
    Bind {
        host: PathBuf,
        #[serde(default, skip_serializing_if = "HashSet::is_empty")]
        flags: HashSet<MountFlag>,
    },
    Dev(Dev),
    Persist(Persist),
}

fn deserialize_tmpfs<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    struct SizeVisitor;

    impl<'de> Visitor<'de> for SizeVisitor {
        type Value = u64;
        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a number of bytes or a string with the size (e.g. 25M)")
        }

        fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E> {
            Ok(v)
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            lazy_static! {
                static ref RE: regex::Regex =
                    regex::Regex::new(r"^(?P<value>\d+)(?P<unit>k|m|g)?$").expect("Invalid regex");
            }

            let caps = RE
                .captures(&v)
                .ok_or_else(|| serde::de::Error::custom(format!("Invalid tmpfs size: {}", v)))?;

            let value = caps
                .name("value")
                .unwrap()
                .as_str()
                .parse::<u64>()
                .map_err(serde::de::Error::custom)?;
            if let Some(unit) = caps.name("unit") {
                let value = match unit.as_str() {
                    "k" => value * 1024,
                    "m" => value * 1024 * 1024,
                    "g" => value * 1024 * 1024 * 1024,
                    _ => {
                        return Err(serde::de::Error::custom(format!(
                            "Invalid tmpfs unit: {}",
                            unit.as_str()
                        )))
                    }
                };
                Ok(value)
            } else {
                Ok(value)
            }
        }
    }

    deserializer.deserialize_any(SizeVisitor)
}

impl MountsSerialization {
    fn serialize<S>(mounts: &HashMap<PathBuf, Mount>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(mounts.len()))?;
        for (target, mount) in mounts {
            match target.display().to_string().as_str() {
                "/dev" => {
                    if let Mount::Dev { r#type } = mount {
                        map.serialize_entry(&target, &r#type)?;
                    } else {
                        return Err(serde::ser::Error::custom("Invalid mount type on /dev"));
                    }
                }
                target => match mount {
                    Mount::Bind { host, flags } => map.serialize_entry(
                        &target,
                        &MountSource::Bind {
                            host: host.clone(),
                            flags: flags.clone(),
                        },
                    )?,
                    Mount::Dev { r#type: _ } => {
                        return Err(serde::ser::Error::custom(format!(
                            "dev cannot be mounted on {}",
                            target
                        )));
                    }
                    Mount::Persist => {
                        map.serialize_entry(&target, &MountSource::Persist(Persist::Persist))?
                    }
                    Mount::Resource { name, version, dir } => map.serialize_entry(
                        &target,
                        &MountSource::Resource {
                            resource: format!("{}:{}{}", name, version, dir.display()),
                        },
                    )?,
                    Mount::Tmpfs { size } => {
                        map.serialize_entry(&target, &MountSource::Tmpfs { tmpfs: *size })?
                    }
                },
            }
        }
        map.end()
    }

    fn deserialize<'de, D>(deserializer: D) -> Result<HashMap<PathBuf, Mount>, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct MountVectorVisitor;
        impl<'de> Visitor<'de> for MountVectorVisitor {
            type Value = HashMap<PathBuf, Mount>;

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut entries = HashMap::<PathBuf, Mount>::new();
                while let Some((target, source)) = map.next_entry()? {
                    let target: PathBuf = target;
                    let mount = match target.display().to_string().as_str() {
                        "/dev" => {
                            if let MountSource::Dev(dev) = source {
                                Mount::Dev { r#type: dev }
                            } else {
                                return Err(serde::de::Error::custom(format!(
                                    "Invalid mount on /dev: {:?}",
                                    source
                                )));
                            }
                        }
                        _ => match source {
                            MountSource::Bind { host, flags } => Mount::Bind { host, flags },
                            MountSource::Dev(..) => {
                                return Err(serde::de::Error::custom(format!(
                                    "dev cannot be mounted on {}",
                                    target.display()
                                )));
                            }
                            MountSource::Tmpfs { tmpfs: size } => Mount::Tmpfs { size },
                            MountSource::Persist(Persist::Persist) => {
                                if entries.values().any(|v| v == &Mount::Persist) {
                                    return Err(serde::de::Error::custom(
                                        "mount configurations can only have one persist entry",
                                    ));
                                }
                                Mount::Persist
                            }
                            MountSource::Resource { resource } => {
                                lazy_static! {
                                    static ref RE: regex::Regex = regex::Regex::new(
                                        r"(?P<name>((\w|-|\.|_)+)):(?P<version>\d+\.\d+\.\d+)(?P<dir>[\w/]+)"
                                    )
                                    .expect("Invalid regex");
                                }

                                let caps = RE.captures(&resource).ok_or_else(|| {
                                    serde::de::Error::custom(format!(
                                        "Invalid resource: {}",
                                        resource
                                    ))
                                })?;

                                let name = caps.name("name").unwrap().as_str().to_string();
                                let version =
                                    Version::parse(caps.name("version").unwrap().as_str())
                                        .map_err(serde::de::Error::custom)?;
                                let dir = PathBuf::from(caps.name("dir").unwrap().as_str());

                                Mount::Resource { name, version, dir }
                            }
                        },
                    };
                    if entries.insert(target.clone(), mount).is_some() {
                        return Err(serde::de::Error::custom(format!(
                            "Duplicate mountpoint: {:?}",
                            target
                        )));
                    }
                }
                Ok(entries)
            }

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> ::std::fmt::Result {
                formatter.write_str("{ /path/a: Bind {} | Persist {} | Resource {}, /path/b: ... }")
            }
        }

        deserializer.deserialize_map(MountVectorVisitor)
    }
}

#[derive(Error, Debug)]
pub enum ManifestError {
    #[error("Invalid manifest ({0})")]
    Invalid(String),
    #[error("Failed to parse: {0}")]
    Parse(#[from] serde_yaml::Error),
    #[error("IO: {0}")]
    Io(#[from] io::Error),
}

impl Manifest {
    fn verify(&self) -> Result<(), ManifestError> {
        // TODO: check for none on env, autostart, cgroups, seccomp, instances
        if self.init.is_none() && self.args.is_some() {
            return Err(ManifestError::Invalid(
                "Arguments not allowed in resource container".to_string(),
            ));
        }
        Ok(())
    }
}

impl FromStr for Manifest {
    type Err = ManifestError;
    fn from_str(s: &str) -> Result<Manifest, ManifestError> {
        let parse_res: Result<Manifest, ManifestError> =
            serde_yaml::from_str(s).map_err(ManifestError::Parse);
        if let Ok(manifest) = &parse_res {
            manifest.verify()?;
        }
        parse_res
    }
}

#[cfg(test)]
mod tests {
    use crate::manifest::*;
    use anyhow::{anyhow, Result};

    #[test]
    fn parse() -> Result<()> {
        let manifest = "
name: hello
version: 0.0.0
init: /binary
args:
  - one
  - two
env:
  LD_LIBRARY_PATH: /lib
mounts:
  /tmp:
    tmpfs: 42
  /dev: full
  /lib:
    host: /lib
    flags:
      - rw
  /data: persist
  /resource:
    resource: bla-blah.foo:1.0.0/bin/foo
autostart: true
cgroups:
  mem:
    limit: 30
  cpu:
    shares: 100
seccomp:
    fork: 1
    waitpid: 1
log:
    tag: test
    buffer:
        custom: 8
";

        let manifest = Manifest::from_str(&manifest)?;

        assert_eq!(manifest.init, Some(PathBuf::from("/binary")));
        assert_eq!(manifest.name, "hello");
        let args = manifest.args.ok_or_else(|| anyhow!("Missing args"))?;
        assert_eq!(args.len(), 2);
        assert_eq!(args[0], "one");
        assert_eq!(args[1], "two");

        assert!(manifest.autostart.unwrap());
        let env = manifest.env.ok_or_else(|| anyhow!("Missing env"))?;
        assert_eq!(
            env.get("LD_LIBRARY_PATH"),
            Some("/lib".to_string()).as_ref()
        );
        let mut mounts = HashMap::new();
        mounts.insert(
            PathBuf::from("/lib"),
            Mount::Bind {
                host: PathBuf::from("/lib"),
                flags: [MountFlag::Rw].iter().cloned().collect(),
            },
        );
        mounts.insert(PathBuf::from("/data"), Mount::Persist);
        mounts.insert(
            PathBuf::from("/resource"),
            Mount::Resource {
                name: "bla-blah.foo".to_string(),
                version: Version::parse("1.0.0")?,
                dir: PathBuf::from("/bin/foo"),
            },
        );
        mounts.insert(PathBuf::from("/tmp"), Mount::Tmpfs { size: 42 });
        mounts.insert(PathBuf::from("/dev"), Mount::Dev { r#type: Dev::Full });
        assert_eq!(manifest.mounts, mounts);
        assert_eq!(
            manifest.cgroups,
            Some(CGroups {
                mem: Some(CGroupMem { limit: 30 }),
                cpu: Some(CGroupCpu { shares: 100 }),
            })
        );

        let mut seccomp = HashMap::new();
        seccomp.insert("fork".to_string(), "1".to_string());
        seccomp.insert("waitpid".to_string(), "1".to_string());
        assert_eq!(manifest.seccomp, Some(seccomp));

        Ok(())
    }

    /// Two mounts on the same target are invalid
    #[test]
    fn duplicate_mount() -> Result<()> {
        let manifest = "
name: hello
version: 0.0.0
init: /binary
mounts:
  /dev: full 
  /dev: full 
";
        assert!(Manifest::from_str(manifest).is_err());

        Ok(())
    }

    #[test]
    fn tmpfs() {
        let manifest = "
name: hello
version: 0.0.0
init: /binary
mounts:
  /a:
    tmpfs: 100
  /b:
    tmpfs: 100k
  /c:
    tmpfs: 100m
  /d:
    tmpfs: 100g
";
        let manifest = Manifest::from_str(manifest).unwrap();
        assert!(manifest.mounts.get(&PathBuf::from("/a")) == Some(&Mount::Tmpfs { size: 100 }));
        assert!(
            manifest.mounts.get(&PathBuf::from("/b")) == Some(&Mount::Tmpfs { size: 100 * 1024 })
        );
        assert!(
            manifest.mounts.get(&PathBuf::from("/c"))
                == Some(&Mount::Tmpfs {
                    size: 100 * 1024 * 1024
                })
        );
        assert!(
            manifest.mounts.get(&PathBuf::from("/d"))
                == Some(&Mount::Tmpfs {
                    size: 100 * 1024 * 1024 * 1024
                })
        );

        // Test a invalid tmpfs size string
        let manifest = "
name: hello
version: 0.0.0
init: /binary
mounts:
  /tmp:
    tmpfs: 100M
";
        assert!(Manifest::from_str(manifest).is_err());
    }

    #[test]
    fn invalid_dev() {
        let manifest = "
name: hello
version: 0.0.0
init: /binary
mounts:
  /dev:
    tmpfs: 42
";
        assert!(Manifest::from_str(manifest).is_err());
    }

    #[test]
    fn mount_ressource() {
        let manifest = "
name: hello
version: 0.0.0
init: /binary
mounts:
  /foo:
    resource: foo-bar.qwerty12:0.0.1/
";
        Manifest::from_str(manifest).unwrap();
    }

    #[test]
    fn serialize_back_and_forth() -> Result<()> {
        let m = "
name: hello
version: 0.0.0
init: /binary
args:
  - one
  - two
env:
  LD_LIBRARY_PATH: /lib
mounts:
  /lib:
    host: /lib
    flags:
      - rw
  /data: persist
  /resource:
    resource: bla-bar.blah1234:1.0.0/bin/foo
  /tmp:
    tmpfs: 42
  /dev: full
autostart: true
cgroups:
  mem:
    limit: 30
  cpu:
    shares: 100
seccomp:
  fork: 1
  waitpid: 1
log:
  tag: test
  buffer:
    custom: 8
";

        let manifest = serde_yaml::from_str::<Manifest>(m)?;
        let deserialized = serde_yaml::from_str::<Manifest>(&serde_yaml::to_string(&manifest)?)?;

        assert_eq!(manifest, deserialized);
        Ok(())
    }

    #[test]
    fn version() -> Result<()> {
        let v1 = Version::parse("1.0.0")?;
        let v2 = Version::parse("2.0.0")?;
        let v3 = Version::parse("3.0.0")?;
        assert!(v2 > v1);
        assert!(v2 < v3);
        let v1_1 = Version::parse("1.1.0")?;
        assert!(v1_1 > v1);
        let v1_1_1 = Version::parse("1.1.1")?;
        assert!(v1_1_1 > v1_1);
        Ok(())
    }
}
