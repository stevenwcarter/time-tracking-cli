Here are some recent changes:
## [0.7.0](https://github.com/stevenwcarter/time-tracking-cli/compare/v0.6.0...v0.7.0) (2025-11-10)


### Features

* add CLI option to accept input from stdin and just generate the report ([b312df5](https://github.com/stevenwcarter/time-tracking-cli/commits/b312df57636a266498327cc541712e7c8c28ae39))
* added bar chart for week hours and ability to zoom ([f49ee65](https://github.com/stevenwcarter/time-tracking-cli/commits/f49ee656963370949848ec546d55701e6268b493))
* added neovim live preview plugin and configuration ([4684f16](https://github.com/stevenwcarter/time-tracking-cli/commits/4684f16bd03e7dd60080de7830550f5eecb6b9c5))
* formatter can be configured in config file now ([ed83361](https://github.com/stevenwcarter/time-tracking-cli/commits/ed83361b1d14613978e5d4ede9d1150b6d82f148))


### Bug Fixes

* fixed file logging implementation ([7f04a60](https://github.com/stevenwcarter/time-tracking-cli/commits/7f04a609add09b47d5ec7ad439b77109fd05c400))


### Chore

* added output to inform user that other jobs are still running ([75329ab](https://github.com/stevenwcarter/time-tracking-cli/commits/75329abbcef5652f07bf648b830bf834371914b2))
* added the new key to the help popup ([cdea99b](https://github.com/stevenwcarter/time-tracking-cli/commits/cdea99be0ad9a9dd679c4ba7c8ba42ba4c97d93d))
* cleaned up imports ([3a036f4](https://github.com/stevenwcarter/time-tracking-cli/commits/3a036f49b71f86c343467ab67580d21648baade6))
* passing args to config for cohesive internal usage ([79fdab3](https://github.com/stevenwcarter/time-tracking-cli/commits/79fdab308891797736db55d2b383306dbec4b02a))
* restructured widgets into their own module ([ffff596](https://github.com/stevenwcarter/time-tracking-cli/commits/ffff5963dd27af1c8bd3b61d7d767f6baa007265))

## [0.6.0](https://github.com/stevenwcarter/time-tracking-cli/compare/v0.5.4...v0.6.0) (2025-11-08)


### Features

* added help popup, popup widget, and reload event ([3cfe52c](https://github.com/stevenwcarter/time-tracking-cli/commits/3cfe52c933dfca0c49235b490c0e2419c01b602e))
* added initial tui implementation ([6e9c687](https://github.com/stevenwcarter/time-tracking-cli/commits/6e9c6876a796dffa254896951b2f7a7875b5aee1))
* added tui with selection and copy/paste on enter ([c59f99f](https://github.com/stevenwcarter/time-tracking-cli/commits/c59f99f361e02453758d323b1d2c9a1bdd987d95))
* dates with data are now highlighted in the calendar view ([3ef7269](https://github.com/stevenwcarter/time-tracking-cli/commits/3ef72691281bc56ce7b04f17404723ba37ecf409))
* enabled editing specific day notes from within tui ([360aa82](https://github.com/stevenwcarter/time-tracking-cli/commits/360aa824587d2362da5cad1598272cf9bc34a826))
* server closes on tui quit ([029ae80](https://github.com/stevenwcarter/time-tracking-cli/commits/029ae80c61f955f960f4fb51c3df4b895b40efd3))
* tui and server can run simultaneously ([fe598e5](https://github.com/stevenwcarter/time-tracking-cli/commits/fe598e5c7fbc8ac9d745ce1b269b2f01ac71d99e))


### Bug Fixes

* removed stderr output breaking tui ([0d3bf9a](https://github.com/stevenwcarter/time-tracking-cli/commits/0d3bf9abdae71dd49dbfd0d79bbb0b72774d2901))


### Chore

* updated justfile to include changelog generator ([96dc94c](https://github.com/stevenwcarter/time-tracking-cli/commits/96dc94cd1038dd572b16c118f872d993524a7baf))


### Docs

* adding documentation and removing unused code ([7a2a6db](https://github.com/stevenwcarter/time-tracking-cli/commits/7a2a6db2cd5b758c3936a115d0a00b0853b6b7a6))


### Style

* separated calendar into own widget, prepping for populated dates ([ae47307](https://github.com/stevenwcarter/time-tracking-cli/commits/ae47307aeaa834601c4c597fae9588f8164a5905))

### [0.5.4](https://github.com/stevenwcarter/time-tracking-cli/compare/v0.5.3...v0.5.4) (2025-11-06)


### Features

* added optional prefix/suffix ([c1b97cc](https://github.com/stevenwcarter/time-tracking-cli/commits/c1b97cce91a3c0df6001464dbd133475172862b7))


### Docs

* expanded detail in README.md for new users ([7f36e85](https://github.com/stevenwcarter/time-tracking-cli/commits/7f36e85dd0cc7fa2540618dd8282aff4f3cbbf8a))


### Chore

* added build script to re-run `yarn build` ([4a020dd](https://github.com/stevenwcarter/time-tracking-cli/commits/4a020ddf28d4c17c3b405ac006ef7a67ed816c9e))
* fixed windows build extraction step ([419c0d3](https://github.com/stevenwcarter/time-tracking-cli/commits/419c0d320a2088be0f76e88dce4e774fca3d2fc3))
* updated page title ([c37b761](https://github.com/stevenwcarter/time-tracking-cli/commits/c37b761e64869d0ff58c8981a782ded95cfee7a0))

### [0.5.3](https://github.com/stevenwcarter/time-tracking-cli/compare/v0.5.2...v0.5.3) (2025-10-05)


### Chore

* added manual workflow dispatch for releases ([b712eb9](https://github.com/stevenwcarter/time-tracking-cli/commits/b712eb91dca3e1e9e6cef843cf47e417202f92ab))
* adding package permissions to build for releases ([85a6e0e](https://github.com/stevenwcarter/time-tracking-cli/commits/85a6e0ef8ef2e0e43c34bc318ed39ffb8fd644cf))
* adding windows build to releases ([db04a80](https://github.com/stevenwcarter/time-tracking-cli/commits/db04a80bdb5bf68dbf1651ff671933add1e6763b))
* bumping build node version to 22 ([06b7c10](https://github.com/stevenwcarter/time-tracking-cli/commits/06b7c10c70e4a1cac8a16cf252f689c9536a7072))

### 0.5.2 (2025-10-05)


### Features

* Added different formatters and restructured ([038dc75](https://github.com/stevenwcarter/time-tracking-cli/commits/038dc756e976c46b9aa3db9306e48e5a7caf424b))
* Added more navigation from week to day and back ([ca195fc](https://github.com/stevenwcarter/time-tracking-cli/commits/ca195fc7fca0841d2ec9a2ae089c8a7563cbac28))
* Added template file configuration for new dates ([6b0ee5f](https://github.com/stevenwcarter/time-tracking-cli/commits/6b0ee5fa854aad8cd2774b744673418376506aad))
* Added toml config and configurable week start day ([e0a4c46](https://github.com/stevenwcarter/time-tracking-cli/commits/e0a4c46d4fd0338b001c7519f899f0eee2924f1f))
* Added week summaries ([4df703f](https://github.com/stevenwcarter/time-tracking-cli/commits/4df703fb3fa4fd43994869e4a35a301069d12d59))
* Added weekly summary and date selector pagination ([1319059](https://github.com/stevenwcarter/time-tracking-cli/commits/131905964577d9c41b23a9e201f3316f65cc4689))
* Adding graphql structure and test impls ([9dced13](https://github.com/stevenwcarter/time-tracking-cli/commits/9dced13e6d3c502f5f712ee7d443ef9fe20211d7))
* Adding navbar ([aa09300](https://github.com/stevenwcarter/time-tracking-cli/commits/aa093002faf26af36b1c2020638917de81fe22f4))
* Adding web server and changing to ttcli binary name ([e51753d](https://github.com/stevenwcarter/time-tracking-cli/commits/e51753d80c94444fa43539883f7711bc8f44c867))
* Allowed date as positional argument ([28e6411](https://github.com/stevenwcarter/time-tracking-cli/commits/28e64112f6920cae0ce88b837519cd2c64c23116))
* Editor experience with date picker ([1264c7a](https://github.com/stevenwcarter/time-tracking-cli/commits/1264c7acff1832fbd6ce57017a01f4243b233fc5))
* Making data directory configurable and adding --noedit option ([d33466f](https://github.com/stevenwcarter/time-tracking-cli/commits/d33466f1a138e68c960b26312b60eebc7a8ce89d))
* Starting to add web editor capabilities ([59df55c](https://github.com/stevenwcarter/time-tracking-cli/commits/59df55cad305c80da4792491436cc2ef3d79a69e))
* Switching to React site in web server ([b3857ca](https://github.com/stevenwcarter/time-tracking-cli/commits/b3857caab8ea9e532c000e1c54fc7046117416f0))
* Updating install script and tweaking installation instructions in README ([dacbdb1](https://github.com/stevenwcarter/time-tracking-cli/commits/dacbdb1b829d19e8c4c8f4c1b3cb858f70dc533b))


### Bug Fixes

* edits now properly clear cache for weekly summary ([d0a0e49](https://github.com/stevenwcarter/time-tracking-cli/commits/d0a0e497f0cd707db31137348488772a8745438b))


### Style

* Allow rounded override on button ([bdfa4e8](https://github.com/stevenwcarter/time-tracking-cli/commits/bdfa4e85db85438d074451fddc47064ad4861bfb))
* Fixing warnings display ([2883fca](https://github.com/stevenwcarter/time-tracking-cli/commits/2883fca0bdf66e791185c32aa3d9fa3f92217a1a))


### Docs

* Adding example video ([52d0181](https://github.com/stevenwcarter/time-tracking-cli/commits/52d018148a71edf56044a584b6e96545de536243))
* Explaining how to launch web server ([03cb790](https://github.com/stevenwcarter/time-tracking-cli/commits/03cb790ca23e5583ed1e9bd5548f5212a53e8d82))


### Chore

* added github action for releases ([65dd8f9](https://github.com/stevenwcarter/time-tracking-cli/commits/65dd8f9460e1a4d465e83c6fb29a11b30887967d))
* added standard-version for releases ([37c73d3](https://github.com/stevenwcarter/time-tracking-cli/commits/37c73d3baa385cab8fab914b35ce37336df894ca))
* Adding release configuration for cargo-release ([ea39b7f](https://github.com/stevenwcarter/time-tracking-cli/commits/ea39b7fb98d86fa64ac791dba99f1831260851a1))
* Formatting code files with rustfmt ([c852d75](https://github.com/stevenwcarter/time-tracking-cli/commits/c852d75ba83f179e3323d4980f06ea6fb4eb8e22))
* made build more efficient ([a4aa12a](https://github.com/stevenwcarter/time-tracking-cli/commits/a4aa12a3084a6235fd324c51bca774d88698d3c8))
* **release:** 0.5.1 ([e462f1b](https://github.com/stevenwcarter/time-tracking-cli/commits/e462f1b4b5bb9e0f791934f18c92b722e69f4127))
* Updating lock file for parser version ([dd8321b](https://github.com/stevenwcarter/time-tracking-cli/commits/dd8321bb2172f549ab06a598d9b4c92719534017))
* Updating TODO.md ([b72f4bd](https://github.com/stevenwcarter/time-tracking-cli/commits/b72f4bd0a94655a824d65e749f94862a0b5e9db7))

### 0.5.1 (2025-10-05)


### Features

* Added different formatters and restructured ([038dc75](https://github.com/stevenwcarter/time-tracking-cli/commits/038dc756e976c46b9aa3db9306e48e5a7caf424b))
* Added more navigation from week to day and back ([ca195fc](https://github.com/stevenwcarter/time-tracking-cli/commits/ca195fc7fca0841d2ec9a2ae089c8a7563cbac28))
* Added template file configuration for new dates ([6b0ee5f](https://github.com/stevenwcarter/time-tracking-cli/commits/6b0ee5fa854aad8cd2774b744673418376506aad))
* Added toml config and configurable week start day ([e0a4c46](https://github.com/stevenwcarter/time-tracking-cli/commits/e0a4c46d4fd0338b001c7519f899f0eee2924f1f))
* Added week summaries ([4df703f](https://github.com/stevenwcarter/time-tracking-cli/commits/4df703fb3fa4fd43994869e4a35a301069d12d59))
* Added weekly summary and date selector pagination ([1319059](https://github.com/stevenwcarter/time-tracking-cli/commits/131905964577d9c41b23a9e201f3316f65cc4689))
* Adding graphql structure and test impls ([9dced13](https://github.com/stevenwcarter/time-tracking-cli/commits/9dced13e6d3c502f5f712ee7d443ef9fe20211d7))
* Adding navbar ([aa09300](https://github.com/stevenwcarter/time-tracking-cli/commits/aa093002faf26af36b1c2020638917de81fe22f4))
* Adding web server and changing to ttcli binary name ([e51753d](https://github.com/stevenwcarter/time-tracking-cli/commits/e51753d80c94444fa43539883f7711bc8f44c867))
* Allowed date as positional argument ([28e6411](https://github.com/stevenwcarter/time-tracking-cli/commits/28e64112f6920cae0ce88b837519cd2c64c23116))
* Editor experience with date picker ([1264c7a](https://github.com/stevenwcarter/time-tracking-cli/commits/1264c7acff1832fbd6ce57017a01f4243b233fc5))
* Making data directory configurable and adding --noedit option ([d33466f](https://github.com/stevenwcarter/time-tracking-cli/commits/d33466f1a138e68c960b26312b60eebc7a8ce89d))
* Starting to add web editor capabilities ([59df55c](https://github.com/stevenwcarter/time-tracking-cli/commits/59df55cad305c80da4792491436cc2ef3d79a69e))
* Switching to React site in web server ([b3857ca](https://github.com/stevenwcarter/time-tracking-cli/commits/b3857caab8ea9e532c000e1c54fc7046117416f0))
* Updating install script and tweaking installation instructions in README ([dacbdb1](https://github.com/stevenwcarter/time-tracking-cli/commits/dacbdb1b829d19e8c4c8f4c1b3cb858f70dc533b))


### Style

* Allow rounded override on button ([bdfa4e8](https://github.com/stevenwcarter/time-tracking-cli/commits/bdfa4e85db85438d074451fddc47064ad4861bfb))
* Fixing warnings display ([2883fca](https://github.com/stevenwcarter/time-tracking-cli/commits/2883fca0bdf66e791185c32aa3d9fa3f92217a1a))


### Docs

* Adding example video ([52d0181](https://github.com/stevenwcarter/time-tracking-cli/commits/52d018148a71edf56044a584b6e96545de536243))
* Explaining how to launch web server ([03cb790](https://github.com/stevenwcarter/time-tracking-cli/commits/03cb790ca23e5583ed1e9bd5548f5212a53e8d82))


### Chore

* added standard-version for releases ([37c73d3](https://github.com/stevenwcarter/time-tracking-cli/commits/37c73d3baa385cab8fab914b35ce37336df894ca))
* Adding release configuration for cargo-release ([ea39b7f](https://github.com/stevenwcarter/time-tracking-cli/commits/ea39b7fb98d86fa64ac791dba99f1831260851a1))
* Formatting code files with rustfmt ([c852d75](https://github.com/stevenwcarter/time-tracking-cli/commits/c852d75ba83f179e3323d4980f06ea6fb4eb8e22))
* Updating lock file for parser version ([dd8321b](https://github.com/stevenwcarter/time-tracking-cli/commits/dd8321bb2172f549ab06a598d9b4c92719534017))
* Updating TODO.md ([b72f4bd](https://github.com/stevenwcarter/time-tracking-cli/commits/b72f4bd0a94655a824d65e749f94862a0b5e9db7))
