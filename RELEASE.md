This lists some of the instructions for releasing the official build of cheese-paper, and requires write access to the repo on codeberg. If you are trying to release a fork of cheese-paper, you will need to have equivalent CI set up (good luck). 

1. Ensure that CI job for the latest commit succeeded (sort out any non-release issues before trying to create the release)

2. Double check that the version in `Cargo.toml` matches the planned version tag (it should! Fix it if necessary)

3. Push version tag:
```
git tag 0.3.0
git push origin 0.3.0
```

4. Ensure *release* job succeeds

5. Check that all artifacts have been uploaded successfully. This should not be a problem if all CI jobs have run to completion (but double check that there isn't still a job pending). Notably, notarization on MacOS might take non-trivial time.

6. Edit release draft and put release notes in

7. Click publish release button

8. Bump version on Cargo.toml to expected next release (second digit bump/minor version, third digit is for patches)
