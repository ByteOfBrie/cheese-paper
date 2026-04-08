use git2::{Repository, RepositoryInitOptions, Signature};
use std::fmt::{Debug, Formatter};
use std::fs::create_dir;
use std::path::PathBuf;
use std::time::Instant;

use crate::ui::prelude::Project;

const COMMITTER_NAME: &str = "Cheese Paper Autosave";
const COMMITTER_EMAIL: &str = "\".\"";

pub struct ProjectTracker {
    repo: Repository,
    pub snapshot_time: Instant,
}

impl Debug for ProjectTracker {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Repo")
            .field("Path", &self.repo.path())
            .field(
                "Head",
                &self
                    .repo
                    .head()
                    .map(|head| head.name().unwrap_or_default().to_string()),
            )
            .finish()
    }
}

impl ProjectTracker {
    pub fn new(project: &Project, data_directory: PathBuf) -> Result<Self, String> {
        let tracker_path = data_directory.join("git_repos");
        // Create data_dir/git_repos/ if it doesn't already exist
        if !tracker_path.exists()
            && let Err(err) = create_dir(tracker_path.as_path())
        {
            return Err(format!(
                "could not create tracker path in data: {tracker_path:?}: {err}"
            ));
        }

        let repo_path = tracker_path.join(format!("{}.git", project.base_metadata.id));

        let repo = if repo_path.exists() {
            match Repository::open(repo_path) {
                Ok(repo) => {
                    if let Err(err) = repo.set_workdir(&project.get_path(), false) {
                        return Err(format!("could not load workdir: {err}"));
                    }
                    repo
                }
                Err(err) => return Err(err.to_string()),
            }
        } else {
            let mut options = RepositoryInitOptions::new();
            options.workdir_path(&project.get_path());
            options.no_dotgit_dir(true);

            match Repository::init_opts(repo_path, &options) {
                Ok(repo) => repo,
                Err(err) => return Err(err.to_string()),
            }
        };

        match repo.head_detached() {
            Ok(true) => {
                log::warn!("Attempting to open a tracker on a detached head");

                if let Err(err) = repo.set_head("main") {
                    return Err(format!("failed to set tracker head to main: {err}"));
                }
            }
            Ok(false) => {} // good state, nothing to do
            Err(err) => return Err(format!("failed to get state of tracker repo head: {err}")),
        }

        let needs_initial_commit = match repo.head() {
            Ok(head) => match head.peel_to_commit() {
                Ok(_commit) => false,
                Err(err) => {
                    log::debug!("Could not peel back first commit: {err}");
                    true
                }
            },
            Err(err) => {
                log::debug!("failed to get tracker head: {err}");
                true
            }
        };

        if needs_initial_commit {
            // no initial commit, we need to create one
            log::debug!("Attempting to create initial commit");

            let committer = Signature::now(COMMITTER_NAME, COMMITTER_EMAIL).unwrap();

            let current_index_tree_oid = match repo.index().and_then(|mut index| index.write_tree())
            {
                Ok(oid) => oid,
                Err(err) => return Err(format!("failed to get tree oid: {err}")),
            };

            let tree = match repo.find_tree(current_index_tree_oid) {
                Ok(tree) => tree,
                Err(err) => return Err(format!("failed to get tree: {err}")),
            };

            if let Err(err) = repo.commit(
                Some("HEAD"),
                &committer,
                &committer,
                "initial commit",
                &tree,
                &Vec::new(),
            ) {
                return Err(format!("failed to create initial commit: {err}"));
            }
        }

        // match repo.head again to see if we fixed it
        match repo.head() {
            Ok(head) => match head.peel_to_commit() {
                Ok(_commit) => {}
                Err(err) => return Err(format!("Could not peel back first commit: {err}")),
            },
            Err(err) => return Err(format!("failed to get tracker head: {err}")),
        }

        Ok(Self {
            repo,
            snapshot_time: Instant::now(), // technically untrue, but we're going to snapshot right away anyway
        })
    }

    pub fn snapshot(&mut self, reason: &str) -> Result<bool, String> {
        self.snapshot_time = Instant::now();

        let committer = Signature::now(COMMITTER_NAME, COMMITTER_EMAIL).unwrap();

        let mut index = self
            .repo
            .index()
            .map_err(|base_err| format!("Cannot get the Index file: {base_err}"))?;

        if let Err(err) = index.add_all(["."], git2::IndexAddOption::DEFAULT, None) {
            return Err(format!("failed to add paths: {err}"));
        }

        // write the index/staging area
        match index.write() {
            Ok(()) => {}
            Err(err) => {
                return Err(format!("failed to write index: {err}"));
            }
        }

        let head_tree = self.repo.head().unwrap().peel_to_tree().unwrap();

        match self
            .repo
            .diff_tree_to_index(Some(&head_tree), Some(&index), None)
        {
            Ok(diff) => {
                if diff.deltas().len() == 0 {
                    log::debug!("Not writing to tracker because no changes are staged");
                    return Ok(false);
                }
            }
            Err(err) => {
                return Err(format!("failed to get diff: {err}"));
            }
        }

        // write the tree that will be used in the commit
        let tree_oid = match index.write_tree() {
            Ok(tree) => tree,
            Err(err) => return Err(format!("failed to write tree: {err}")),
        };

        let tree = self.repo.find_tree(tree_oid).unwrap();

        let parent_commit = self
            .repo
            .head()
            .map_err(|base_err| {
                format!("Error when loading parent commit: head does not exist: {base_err}")
            })?
            .peel_to_commit()
            .map_err(|base_err| {
                format!(
                    "Error when loading parent commit: head should always have commits: {base_err}"
                )
            })?;

        if let Err(err) = self.repo.commit(
            Some("HEAD"),
            &committer,
            &committer,
            reason,
            &tree,
            &[&parent_commit],
        ) {
            return Err(format!("failed to create snapshot: {err}"));
        }

        Ok(true)
    }
}
