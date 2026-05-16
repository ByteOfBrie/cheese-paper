Thank you for using Cheese Paper. The code currently lives at https://codeberg.org/ByteOfBrie/cheese-paper, and is automatically mirrored to https://github.com/ByteOfBrie/cheese-paper (for now). If you have issues, suggestions, or other feedback, please open an issue on Codeberg (GitHub issues may or may not also be looked at)

# Basic Flow

Cheese Paper is intended to be used for writing stories which can be broken down into smaller components.

There are currently four types of objects in Cheese Paper:

* Scenes contain the text of the story. This can be as much as an entire chapter, or as little as a paragraph, it is entirely up to the author. Scenes have a summary and notes for the author's convenience, and are not included in the final export.
* Folders are used to store any type of object. These can also contain a summary and notes.
* Characters are purely so the author has a a handy place to fill out some information about who is in the story (so they don't forget a character's hair color or the spelling of their middle name on chapter seventeen).
* Places are similar to characters, but intended for information about the world. This can be a specific place/area, or worldbuilding information. Once again, this is purely for organization and can be used in whatever way the author works.

Depending on whether you're working with a scene, folder, character, or place, the options for metadata will be slightly different, but the concept is all the same. You have a couple different fields that you can fill out and see while writing the story.

Ideally, this can help keep the story on track and make it easier to refer back to all of the other details. Can't remember how a specific character would normally dress? It's one click away.

You are encouraged to split up your project into as many folders and scenes as makes sense to you. Once you want to share it, there are options to export an outline or the story text into a single shareable file.

# Key Features

## File Format

One of the key design decisions was keeping the file format as simple as possible. Everything except scenes are [toml](https://toml.io/) documents with a few set fields. Scenes are a toml document, followed by `++++++++`, followed by the text of the scene (markdown). Both of these file formats were designed to be easy for humans to read.

You can edit them yourself by hand! You can edit them on a phone!

## Loading Partial Files

Cheese Paper is designed to have project files be edited, moved, deleted, and created by programs outside of the editor. This can happen on the first load, or if a file is editing or synced to while Cheese Paper is already open

The file format is documented below in more detail. For minimum requirements:

Scene: file that ends in `.md`

Character: must have `file_type = "character"` and end in `.toml`

Folder: must exist as a folder

Place: must exist in a `place_name/metadata.toml` and have `file_type = "place"` within

# Integration

## Spellcheck

By default, Cheese Paper attempts to use spellcheck with en_US (US English). This can be switched to another language or disabled entirely in the settings. The list of available dictionaries will vary by operating systems, but it's easy to add more.

On non-flatpak Linux, Cheese Paper's spellcheck should work out of the box with dictionaries installed by your package manager (Cheese Paper looks for dictionaries in `/usr/share/hunspell/`).

The flatpak Linux Cheese Paper cannot access system dictionaries, but comes with a selection of dictionaries to choose from.

On Windows and MacOS, Cheese Paper comes with a default en_US dictionary. To use other dictionaries in other languages, you will need to download the `.dic` and `.aff` files yourself. They can be downloaded from various places, notably from libreoffice: https://github.com/LibreOffice/dictionaries/

Once you have both a `.dic` and `.aff` file, you'll want to open Cheese Paper settings and clicking the "open spellcheck folder" button in the settings. After placing both the `.dic` and `.aff` files directly in that folder and restarting Cheese Paper, you should be able to select the desired dictionary.

### Per-Project Spellcheck

Spellcheck can also be set per-project in the project settings. If you want to have most projects in English, one in German, and one in French, Cheese Paper won't require you to change settings every time you switch between projects.

## Export

Cheese Paper directories are not very useful for sharing your completed story, which is why you can export. The "export story text" option lets you output the contents of every scene, combined together.

The default export settings assume that top level folders and scenes are should be given headings as if they are chapters, and nothing else should. This can be changed project-wide on the export screen, or this can be changed for a specific scene or folder from their sidebar.

Markdown is designed to be fairly human readable, but you probably don't want to try to publish your story with this. Thankfully, markdown is a well known format and there are plenty of options for converting to other formats. Most notably, pandoc.

Pandoc is an another open-source tool that can deal with all different types of document conversion, including taking your nice markdown file and turning it into an epub, word document, or pdf. See https://pandoc.org/app/ for an easy-to-use in-browser converter.

(Side note: if you are creating a book, please consider not only distributing PDFs. PDFs can be difficult for readers who need larger font sizes, particularly on smaller devices like phones)

## Outline Export

Cheese Paper is designed to hold a lot of information about your writing project inside of the editor itself. This is great for your own use, but some of your friends might not have Cheese Paper (unfortunately), or you might not want to share the entire project file with them.

For this, Cheese Paper has the ability to export the entire outline into a single file. It will put all of the summaries and notes from your project into a markdown file that can then be shared as desired.

## Syncthing (and syncing in general)

syncthing is an open source file synchronization system. Cheese Paper was primarily designed around the specific use case of using syncthing, and is fairly well tested for that use case.

Other programs will likely work well, Cheese Paper's sync model is theoretically fairly resilient, but that has not been (currently) thoroughly exercised. Please report any bugs you encounter here

# Other Notes

## Logs

Cheese Paper will automatically write logs. The exact path will vary based on OS:

Linux: `~/.local/share/cheese-paper/logs`

Windows: `%appdata%\cheese-paper\logs` (or `C:\Users\<User>\AppData\Roaming\cheese-paper\logs`)

MacOS: `~/Library/Application Support/cheese-paper/`

The log levels can be changed if desired by setting the desired `RUST_LOG` variable before starting Cheese Paper (this will vary by platform). The default log level is `warn,cheese_paper=info`, meaning that logs will include warnings and above from other libraries, while Cheese Paper message from info and above will be included.

## Markdown

Markdown is rather complicated and Cheese Paper's rendering only displays a tiny portion of that. However, the intended path is to use cheese paper and then Pandoc, which does implement everything.

Users familiar with markdown can use whatever formatting they like, and it should render without issues in their final product

Users unfamiliar with markdown should be sure to glance over exports to ensure that unintended formatting has not occurred

If there are frequent cases of unexpected formatting confusion, please open an issue on codeberg.

## Full File Format

This section is most likely *not* useful to most users, except for developers trying to work off of this. The manual *should* stay up-to-date with changes, but it's possible that it will lag behind, so for any truly critical changes it may be worth double checking with the source code and/or opening an issue to ask.

Every file starts off with metadata, which has four constant fields:

`version` - a positive integer representing the version of the file format, currently set to `1` everywhere and not parsed further. Can theoretically be used to enable certain types of changes to the format without breaking backwards compatibility

`name` - a string representing the name of the file object (e.g., the character's name, the title of a scene, the name of a place). The string should be quoted in accordance with the toml spec

`id` - unique identifier for this object as a string. All generated IDs are UUIDv4 strings, but this is not at all required. IDs should *not* contain `|` or `]` characters, this will break other functionality. If the ID of a file is changed, all references to it. There may be strange behavior if this is changed while Cheese Paper is running

`file_type` - string specifier of the type of file_object, used when parsing

All file types have some metadata fields that are set specifically, most commonly a `notes` section, which is a freeform text box.

After the header, scenes will have a divider, `++++++++`, and then the body of the scene directly.

Tooling on top of Cheese Paper should be possible (and very neat), feel free to open an issue to ask questions about anything that is unclear

## Git Repos

Cheese Paper will track your files in git automatically inside of it's data directory. You don't have to do anything to enable this, and it will attempt to take a snapshot of the state of your project every 15 minutes.

These files will not ever be uploaded anywhere (unless you do this yourself), and they will not ever overwrite the contents of your project (unless you do this yourself).

In the event that you accidentally delete some of your project files, or there is a very severe Cheese Paper bug that does so, the contents of your file *may* be stored in git. Please take regular backups instead of relying on this (it's a good idea regardless, writing is precious). If you are in a situation where you want to recover data from the git repos and you need help, please ask for it before you do something that could accidentally delete data.

### Advanced Git

If you are not already familiar with git, please be somewhat careful with these commands. It shouldn't be easy to lose data, but there's no point in tempting fate.

Cheese Paper itself will eventually(tm) have some features that integrate with the data in the git repo, and it passively exists as history for you if you ever truly need that

#### Git From Within The Repo

Every project will be created with a `.git` file that points at the location of the git repo (which is in a separate folder):

```
brie@cheddar ~/w/cheese-paper-test (main)> cat .git
gitdir: /home/brie/.local/share/cheese-paper/git_repos/2016bc85-57bc-42e5-bdb3-cb47110cada3.git
```

If you have `git` installed and run `git` commands inside of the folder, it will automatically detect the `.git` file and pick it up, no special options are needed.

#### Managing A Separate Git Repo

Some users may want to manage git themselves, without Cheese Paper automatically committing every 15 minutes and resetting the branch when restarted. This is fine, and fully supported by Cheese Paper.

To even create your self-managed git repo, you'll need to remove that file: `rm .git`

This will only affect the ability to run git commands from inside of the folder: Cheese Paper will continue to manage its automatic tracker as normal.

After you have deleted that initial link, you can do `git init` and start managing your local git repo however you wish, Cheese Paper will not interfere.

If you wish to work with the automatic tracker repo after you have removed the `.git` link, you'll need to either set the env variable:
```
GIT_DIR=/home/brie/.local/share/cheese-paper/git_repos/2016bc85-57bc-42e5-bdb3-cb47110cada3.git
```

or pass that to all of your commands:
```
git --git-dir=/home/brie/.local/share/cheese-paper/git_repos/2016bc85-57bc-42e5-bdb3-cb47110cada3.git
```

At this point, you can manage it as normal, and then go back to using your local git repo.

## Ignored Words

If you ignore a word in the spellcheck dialogue, you will not ever see it flagged again in any of your projects. If you do this by mistake, you can open up the data file (`data.toml` in the folder containing the log folder. See the log section for per-platform paths).

First, close Cheese Paper (otherwise we might accidentally add it back in to the list of ignored words)

In the data file, you'll see a `custom_dictionary` value, which will be a list of all of the words that were ever ignored. Delete the word(s) from the list, and the next time you start Cheese Paper, that word will be marked as misspelled again.

## Quirks

(aka bugs that are weird enough to document instead of fixing (so far))

If you put the string `++++++++` in the metadata of a scene (or elsewhere in the header), the editor will be unable to load that file in the future.
