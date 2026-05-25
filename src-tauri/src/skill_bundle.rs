/// Skill Bundle — importable/exportable skill packages with prompts and tool configs.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SkillStep {
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SkillItem {
    pub name: String,
    pub description: String,
    pub prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steps: Option<Vec<SkillStep>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SkillBundle {
    pub name: String,
    pub version: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    pub skills: Vec<SkillItem>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InstalledSkill {
    pub id: String,
    pub bundle_name: String,
    pub name: String,
    pub description: String,
    pub prompt: String,
    pub version: String,
    pub enabled: bool,
}

fn skills_dir() -> Result<PathBuf, String> {
    let base = std::env::var("APPDATA")
        .map(|a| PathBuf::from(a).join("ai-tools-hub"))
        .map_err(|_| String::from("no appdata"))?;
    let dir = base.join("skills");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

/// List all installed skills
pub fn list_installed() -> Vec<InstalledSkill> {
    let dir = match skills_dir() {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };
    let mut skills = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                match std::fs::read_to_string(&path) {
                    Ok(content) => match serde_json::from_str::<SkillBundle>(&content) {
                        Ok(bundle) => {
                            for skill in &bundle.skills {
                                skills.push(InstalledSkill {
                                    id: fmt_id(&bundle.name, &skill.name),
                                    bundle_name: bundle.name.clone(),
                                    name: skill.name.clone(),
                                    description: skill.description.clone(),
                                    prompt: skill.prompt.clone(),
                                    version: bundle.version.clone(),
                                    enabled: true,
                                });
                            }
                        }
                        Err(e) => eprintln!("[skills] Warning: failed to parse {}: {}", path.display(), e),
                    },
                    Err(e) => eprintln!("[skills] Warning: failed to read {}: {}", path.display(), e),
                }
            }
        }
    }
    skills
}

/// Install or upgrade a skill bundle
pub fn install(json: &str) -> Result<Vec<InstalledSkill>, String> {
    let bundle: SkillBundle = serde_json::from_str(json).map_err(|e| format!("Invalid skill: {}", e))?;
    if bundle.skills.is_empty() {
        return Err("Bundle has no skills".into());
    }

    let dir = skills_dir()?;
    let safe_name = bundle.name.replace(' ', "_").to_lowercase();

    // Remove any previous version of the same bundle
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if let Ok(content) = std::fs::read_to_string(&p) {
                if let Ok(existing) = serde_json::from_str::<SkillBundle>(&content) {
                    if existing.name == bundle.name && p.exists() {
                        std::fs::remove_file(&p).ok();
                        eprintln!("[skills] Removed old version {} v{}", bundle.name, existing.version);
                    }
                }
            }
        }
    }

    // Save new version
    let filename = format!("{}-v{}.json", safe_name, bundle.version);
    let path = dir.join(&filename);
    let pretty = serde_json::to_string_pretty(&bundle).map_err(|e| e.to_string())?;
    std::fs::write(&path, pretty).map_err(|e| format!("Save failed: {}", e))?;

    // Return installed skills
    let mut skills = Vec::new();
    for skill in &bundle.skills {
        skills.push(InstalledSkill {
            id: fmt_id(&bundle.name, &skill.name),
            bundle_name: bundle.name.clone(),
            name: skill.name.clone(),
            description: skill.description.clone(),
            prompt: skill.prompt.clone(),
            version: bundle.version.clone(),
            enabled: true,
        });
    }
    Ok(skills)
}

/// Uninstall a skill bundle by name
pub fn uninstall(bundle_name: &str) -> Result<(), String> {
    let dir = skills_dir()?;
    let mut found = false;

    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let content = match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue, // skip unreadable files
            };
            match serde_json::from_str::<SkillBundle>(&content) {
                Ok(bundle) if bundle.name == bundle_name => {
                    std::fs::remove_file(&path).map_err(|e| format!("Delete failed: {}", e))?;
                    eprintln!("[skills] Removed bundle: {}", bundle_name);
                    found = true;
                }
                Ok(_) => {} // different bundle, skip
                Err(e) => {
                    eprintln!("[skills] Skipping non-bundle file {}: {}", path.display(), e);
                }
            }
        }
    }

    if found { Ok(()) } else { Err(format!("Bundle '{}' not found", bundle_name)) }
}

fn fmt_id(bundle: &str, skill: &str) -> String {
    format!("{}_{}", bundle, skill).replace(' ', "_").to_lowercase()
}
