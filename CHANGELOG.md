Here are some recent changes:
## [0.9.0](https://github.com/stevenwcarter/time-tracking-cli/compare/v0.8.0...v0.9.0) (2026-09-02)


### ⚠ BREAKING CHANGES

* (TuiContext::from_config now fails tui() before
  ratatui::init() instead of silently falling back to Saturday).
- weekly_summary_includes_warnings_section's doc comment said it pinned
  'both' warning shapes; it pins three things (the section header plus
  both shapes).
- keymap.rs's ScrollRawFileDown binding said it was disjoint from 'the
  Day-only list-navigation row above', but the week rollup pane has its
  own j/k row too now.

Claude-Session: https://claude.ai/code/session_01T9DGuW5Lm8VkXZF971mAkZ

### Features

* add try_get_no_args() returning Result instead of panicking ([6fade96](https://github.com/stevenwcarter/time-tracking-cli/commits/6fade96ef8745fb4ab646ffb53fee5c7741df54a))
* adding relative dates for arguments ([e27ed76](https://github.com/stevenwcarter/time-tracking-cli/commits/e27ed768193413e3946de2a86689a3d6595f1474))
* formatting hours ([0d516b7](https://github.com/stevenwcarter/time-tracking-cli/commits/0d516b78fcd7817cd356b4bbf81bd5b987b3df03))
* **tui:** act on clicks and wheel scrolling ([18ef28c](https://github.com/stevenwcarter/time-tracking-cli/commits/18ef28c757b1fe9e64b0e7694bd175361e447f29))
* **tui:** add a jump-to-date prompt accepting natural-language dates ([6e528d6](https://github.com/stevenwcarter/time-tracking-cli/commits/6e528d639f9bd59e3a8bf1bd0a0db1fed268a709))
* **tui:** add a status line and move clipboard IO into App ([b473a23](https://github.com/stevenwcarter/time-tracking-cli/commits/b473a23e862063dfad3e391341501b44c751e856))
* **tui:** add a weekly per-project rollup pane ([23d006e](https://github.com/stevenwcarter/time-tracking-cli/commits/23d006e4a2cdc96068de66e0ebb48f3bac38b97e))
* **tui:** add config-driven light, dark, and no-color themes ([f92f3fc](https://github.com/stevenwcarter/time-tracking-cli/commits/f92f3fc62d6754e387857e690a3c590d1054934d))
* **tui:** add week and month date motions ([d4024b7](https://github.com/stevenwcarter/time-tracking-cli/commits/d4024b7e90ccc4503a26c24e38dacee41afffbc3))
* **tui:** auto-refresh when the day file changes on disk ([8eca567](https://github.com/stevenwcarter/time-tracking-cli/commits/8eca5677659a8ec3d30ad3dba734d0ad88013adc))
* **tui:** capture the mouse, governed by a mouse config key ([3c08681](https://github.com/stevenwcarter/time-tracking-cli/commits/3c086812501fba5268b4e7a4012cb15b6465b7a3))
* **tui:** give the empty-date screen a call to action ([098cdfd](https://github.com/stevenwcarter/time-tracking-cli/commits/098cdfd935ee5a21629da59c3f0978b340a9d3e2))
* **tui:** make the layout responsive with a minimum-size notice ([f5c22e1](https://github.com/stevenwcarter/time-tracking-cli/commits/f5c22e1ed3756a6288ef3a59793638c20fc3516d))
* **tui:** open the editor on a double click ([d1bfaf1](https://github.com/stevenwcarter/time-tracking-cli/commits/d1bfaf1ef84b47f95bbca6c80e5976316f04c3a6))
* **tui:** record where each clickable region was drawn ([6feb1db](https://github.com/stevenwcarter/time-tracking-cli/commits/6feb1db132c4c24a5ce1fd5d1cfb86f4ddcee7e4))
* **tui:** resolve a bar chart column to its day ([c01eff5](https://github.com/stevenwcarter/time-tracking-cli/commits/c01eff5731954f54032026cf61e2130a26c5da62))
* **tui:** resolve a calendar cell to its date ([0a2ff9d](https://github.com/stevenwcarter/time-tracking-cli/commits/0a2ff9d465496234bb0bcddb6b1b1ae408127a32))
* **tui:** resolve a list row to its index ([7a1bd5d](https://github.com/stevenwcarter/time-tracking-cli/commits/7a1bd5d1eb8ee05cf65af3018d0323da2a9d0826))
* **tui:** show dead time and parser warnings in the day header ([738b1a2](https://github.com/stevenwcarter/time-tracking-cli/commits/738b1a27c103ea96da7e02358f4f439dec82eee3))
* **tui:** show the active date and weekday on the project pane ([e001f1c](https://github.com/stevenwcarter/time-tracking-cli/commits/e001f1ccb6cae42a73946f2b849224b69a7f0e7b))
* **tui:** suspend to the shell with Ctrl-Z ([c070f74](https://github.com/stevenwcarter/time-tracking-cli/commits/c070f743665decbbf9fea755f7c8177dd06705ba))
* **tui:** view the raw day file without leaving the TUI ([2d81b75](https://github.com/stevenwcarter/time-tracking-cli/commits/2d81b75e4cb48811ff756f78942681b696a32d93))
* **tui:** yank the day or week summary to the clipboard ([17233d9](https://github.com/stevenwcarter/time-tracking-cli/commits/17233d9687d31ec8e8656062aad0b208cac79576))


### Bug Fixes

* address the review's three Minor findings ([7ca4c72](https://github.com/stevenwcarter/time-tracking-cli/commits/7ca4c72f94976e73d54e5b1951f860a509520e85))
* **api-surface:** 404 unmatched /api and /graphql paths [B15] ([8f195c0](https://github.com/stevenwcarter/time-tracking-cli/commits/8f195c0371d8b918246687ede336cb390aac26bb))
* **api-surface:** fail non-zero on an unparseable --date [B12] ([0569e00](https://github.com/stevenwcarter/time-tracking-cli/commits/0569e00be7c8544e0b8fc5efcf811932edc1b5ef))
* **api-surface:** make registered_routes_still_resolve hermetic [B15] ([7ba9a32](https://github.com/stevenwcarter/time-tracking-cli/commits/7ba9a32d0fb26276cdc58772ea8fd56e2c01ffa6))
* **api-surface:** warn when --stdin ignores the other mode flags [B17] ([48ae43a](https://github.com/stevenwcarter/time-tracking-cli/commits/48ae43abf1202f87ddbba0bb9c5d2b1333044434))
* **cache:** do not attach a parse to content it did not come from ([bc196f5](https://github.com/stevenwcarter/time-tracking-cli/commits/bc196f5cdaa52f1710deb3ba1745e18e4d132f92))
* **cache:** treat any change of mtime as a change, not just a later one ([593724e](https://github.com/stevenwcarter/time-tracking-cli/commits/593724ecec003c868959dc8bb36c6e32805bafb9))
* **caching:** clone only the cache field each caller reads [B11] ([d18b386](https://github.com/stevenwcarter/time-tracking-cli/commits/d18b386e894b4faf5594bab3a2ef7f5f625afd3d))
* **caching:** drop the blocking existence stat from parse_day [B3] ([8d043cb](https://github.com/stevenwcarter/time-tracking-cli/commits/8d043cb0b8e18a88e84b9219d65c06961b49c198))
* **caching:** parse day data through the memoized DataService cache [B5] ([89ec087](https://github.com/stevenwcarter/time-tracking-cli/commits/89ec0871246d655d7ebca03a17909a3ffd160ad9))
* **caching:** sweep expired entries instead of only declining them [B13] ([04a3e5b](https://github.com/stevenwcarter/time-tracking-cli/commits/04a3e5b0126203df2b4ee3683aca314a8f36444b))
* **ci:** derive the archive branch name correctly on pull_request events ([40ca894](https://github.com/stevenwcarter/time-tracking-cli/commits/40ca8948d38ea4b4eaed1a895e460fe58691155d))
* clear three pre-existing clippy findings ([e8dab10](https://github.com/stevenwcarter/time-tracking-cli/commits/e8dab10d0efdb39e1332aad733df07e1eef45bdd))
* **cli:** handle webserver_running correctly in all feature combinations ([135d105](https://github.com/stevenwcarter/time-tracking-cli/commits/135d10544f865ac17dd814144b3943bdb3eeecee))
* **cli:** make tx binding conditional on both webapp and tui features ([481e07b](https://github.com/stevenwcarter/time-tracking-cli/commits/481e07b19592fd7d2b717f00588d1d55151c89d0))
* **cli:** only print the webserver hint when the webserver is running ([ff29840](https://github.com/stevenwcarter/time-tracking-cli/commits/ff29840842eaf8191a0c34cd78ac9d307b6fe0ea))
* **cli:** restore sender scope to fix graceful shutdown in webapp-only builds ([4d6fc97](https://github.com/stevenwcarter/time-tracking-cli/commits/4d6fc97a19b82bed58928061d2aba20ab3066bf3))
* **cli:** restore user-visible webserver and stdin-flag diagnostics ([d0539bd](https://github.com/stevenwcarter/time-tracking-cli/commits/d0539bd3c2a484ab594ee7f8f0909a2999af1242))
* **cli:** rewrite runtime smoke test to properly verify shutdown channel ([1de1bc0](https://github.com/stevenwcarter/time-tracking-cli/commits/1de1bc0618574c8621fcaf38b01b20d8901becf4))
* **cli:** stop cli/Cargo.toml from forcing both lib features on every build ([6cde103](https://github.com/stevenwcarter/time-tracking-cli/commits/6cde103cbf1e16c46386e37468b4f34c8678f371))
* code health improvements (10 items) ([73035a2](https://github.com/stevenwcarter/time-tracking-cli/commits/73035a291f32d3a028b0949443ceb5405046ce9a))
* code health improvements (2 items) ([9a50e9d](https://github.com/stevenwcarter/time-tracking-cli/commits/9a50e9d5c67fb70b809e101345ea89f881c34d27))
* code health improvements (6 items) ([142f5ac](https://github.com/stevenwcarter/time-tracking-cli/commits/142f5acdd21d650be056b0fe6a3738fc48769800))
* code health improvements (7 items) ([840e855](https://github.com/stevenwcarter/time-tracking-cli/commits/840e8556d7e80ed23e9f08fdd832ad3bf8a772aa))
* code health improvements (7 items) ([e70c6f6](https://github.com/stevenwcarter/time-tracking-cli/commits/e70c6f667f4013889a9adcf30846d9ada6926c44))
* code health improvements (9 items) ([1d75878](https://github.com/stevenwcarter/time-tracking-cli/commits/1d758786e88e97fb4f2bb43118103e4fc68f95d8))
* **config:** resolve data_directory on load, not only in Default ([966a37b](https://github.com/stevenwcarter/time-tracking-cli/commits/966a37b85cb968affec05443a02c04741c453e38))
* **correctness:** split a multi-word $EDITOR into program and args [B10] ([dbbf822](https://github.com/stevenwcarter/time-tracking-cli/commits/dbbf8226ff96be109364ce3f410b18cb8aa94c45))
* **data-svc:** flush the template write and build content before create ([1e2e848](https://github.com/stevenwcarter/time-tracking-cli/commits/1e2e8481ba279da64a95e7b31fa529924241d9bf))
* **data:** skip one unreadable day file instead of failing the whole scan ([a1e76f8](https://github.com/stevenwcarter/time-tracking-cli/commits/a1e76f8fb480c4da21654578577e15ce2f0e81ad))
* disable ANSI codes in log file (O2) and enable RUST_LOG env var via EnvFilter (O3) ([6845df8](https://github.com/stevenwcarter/time-tracking-cli/commits/6845df89c7b745f7917dcfa3675f433fab8386bc))
* **display:** render the weekly warnings block through the formatter ([2a5405d](https://github.com/stevenwcarter/time-tracking-cli/commits/2a5405de4a5f9210b40aa63867a3040caf78ec71))
* fmt and clippy cleanups for cli_runtime_smoke test ([ebc44b5](https://github.com/stevenwcarter/time-tracking-cli/commits/ebc44b5edebc4502944e13be10c49adf1721b7f9))
* **frontend:** refetch by operation name so every mounted date updates [B7] ([680016a](https://github.com/stevenwcarter/time-tracking-cli/commits/680016a884652f7cc8560fccd55ed82a06855107))
* **frontend:** stop DateEditor re-saving the content it just loaded [B6] ([50e4c38](https://github.com/stevenwcarter/time-tracking-cli/commits/50e4c38ef8a8ed18974b1ccf5701181003bcc7c5))
* **frontend:** surface Apollo query errors instead of discarding them [B8] ([73d188c](https://github.com/stevenwcarter/time-tracking-cli/commits/73d188c2379d696d0e5faf9d10d329ede6e4dfc9))
* guard Apollo queries with !dateString instead of !date (C8) ([094e76f](https://github.com/stevenwcarter/time-tracking-cli/commits/094e76f2b0bf70e7c4f7205e1fce6b0466ef0190))
* import vi from vitest in App.test.tsx to fix tsc build ([fe7f2b2](https://github.com/stevenwcarter/time-tracking-cli/commits/fe7f2b2f81aa832b76ed69a50a7737209539e669))
* **observability:** keep background-task messages off the TUI screen [B9] ([4947a98](https://github.com/stevenwcarter/time-tracking-cli/commits/4947a981d10f507be6287f922bafc4d069dc45c8))
* **observability:** log day-data read failures before collapsing to 500 [B4] ([c141382](https://github.com/stevenwcarter/time-tracking-cli/commits/c1413826c5a391ea615eb539dbeac9b194736010))
* **observability:** report a failed watch retarget on the status line [B16] ([bb83b02](https://github.com/stevenwcarter/time-tracking-cli/commits/bb83b02df327b81ae528c8ae21847598d84cf6dd))
* parse date picker value as local midnight to avoid UTC timezone shift (C7) ([0fc67e9](https://github.com/stevenwcarter/time-tracking-cli/commits/0fc67e9867a91324129ae4da2390f915b995a181))
* remove duplicate React.StrictMode wrapper in App.tsx (C6) ([705701f](https://github.com/stevenwcarter/time-tracking-cli/commits/705701f25f1c76885071ae8bfa4c5ec361883d33))
* remove unreachable else branch in show_single_day (C3) ([e200870](https://github.com/stevenwcarter/time-tracking-cli/commits/e2008701f692a20f8a9a0142ad94165ca701b01c))
* route GraphQL reads through DataService cache and invalidate on write (C1+C2) ([56b1bc2](https://github.com/stevenwcarter/time-tracking-cli/commits/56b1bc2e6058e4b670a524ed1bbbfefb1d6c0bdc))
* **test:** give the TUI launch test no controlling terminal ([20a4a35](https://github.com/stevenwcarter/time-tracking-cli/commits/20a4a350dd9bbffdd39c2038383169cbae0f1411))
* **tui:** bound the header's Working/Dead Time row so it can't truncate ([95b29e4](https://github.com/stevenwcarter/time-tracking-cli/commits/95b29e4f99ae1cda43821a4850408a323d6780b1))
* **tui:** draw the calendar band once the list stays usable, not complete ([ff616d4](https://github.com/stevenwcarter/time-tracking-cli/commits/ff616d4806b7293a139f2d3d64f9bc81b4895cc0))
* **tui:** draw the calendar band only once the list no longer needs its rows ([438e406](https://github.com/stevenwcarter/time-tracking-cli/commits/438e406e6701cb43ae54c4c5c3e462d4ded0e7ec))
* **tui:** floor the total-hours position and the goal-marker row ([6ce6422](https://github.com/stevenwcarter/time-tracking-cli/commits/6ce64221c890a32319e2ca8169b0fab62673d6f6))
* **tui:** gate the day yank and the raw pane on the same freshness check ([a678b82](https://github.com/stevenwcarter/time-tracking-cli/commits/a678b82c8bed4f9a527e3bd4fcadeb800eca41df))
* **tui:** gate Y on week_is_stale and drop the new expect() ([a685c7d](https://github.com/stevenwcarter/time-tracking-cli/commits/a685c7d1ec4502ed784470f0eea6b70d842aa774))
* **tui:** keep App Send so the CLI can spawn the TUI ([f825765](https://github.com/stevenwcarter/time-tracking-cli/commits/f82576531cc650b54d8c48feb342dc2285f7041e))
* **tui:** keep row/selection colours distinct under 16-colour downgrade ([e1d370e](https://github.com/stevenwcarter/time-tracking-cli/commits/e1d370ed5889678af351502ee883979d7665923b))
* **tui:** make `?` reachable on Windows and zoom-key routing synchronous ([1c456e2](https://github.com/stevenwcarter/time-tracking-cli/commits/1c456e2d9767fdf234047634af965b189c66676d))
* **tui:** make both columns of a calendar day clickable ([ea82bfb](https://github.com/stevenwcarter/time-tracking-cli/commits/ea82bfb15227a58c4a31b3a4385f963b170f74d2))
* **tui:** make get_week_dates infallibly return [Date; 7] ([87f7a7f](https://github.com/stevenwcarter/time-tracking-cli/commits/87f7a7fc197ef233aff125c4b6eca3e5f28574d6))
* **tui:** make mouse-capture toggles best-effort so restore is never skipped ([abe14bf](https://github.com/stevenwcarter/time-tracking-cli/commits/abe14bfde97f9c1a244dd542c0f42b2f1f618b57))
* **tui:** mark a queued reload as loading to close the last flash ([b0feaab](https://github.com/stevenwcarter/time-tracking-cli/commits/b0feaab61ee678445cb4ccd66840d55c709c44af))
* **tui:** match Ctrl-Z the way crossterm actually delivers it ([5d3115f](https://github.com/stevenwcarter/time-tracking-cli/commits/5d3115f2288fcec57174f67c4802759ba9672320))
* **tui:** re-clamp raw-file scroll, surface read errors, guard the re-read's generation ([6d82d10](https://github.com/stevenwcarter/time-tracking-cli/commits/6d82d1037ce4af04d4212d32e2851ffae26de6b9))
* **tui:** render the real week pane in the no-panic sweep, show week warnings ([db06042](https://github.com/stevenwcarter/time-tracking-cli/commits/db06042b72d5d76a32e4ab83678a66bcf2d6df39))
* **tui:** resolve list clicks against the rows render actually drew ([b156665](https://github.com/stevenwcarter/time-tracking-cli/commits/b15666555b091358d4e762a17e533435bddb1c6c))
* **tui:** resume the event poller when the editor handover fails ([6c26b06](https://github.com/stevenwcarter/time-tracking-cli/commits/6c26b06accf2db63a200253ee985271aa10113a3))
* **tui:** scale the weekly chart to the data instead of terminal height ([f81bea2](https://github.com/stevenwcarter/time-tracking-cli/commits/f81bea2410d2ee6377493f52532178d9830f364f))
* **tui:** stop hiding quit from the help popup, add Ctrl-C row (R29) ([ab3c45b](https://github.com/stevenwcarter/time-tracking-cli/commits/ab3c45b3b869f3c7847c7431c2a83b5782d0663a))
* **tui:** stop testing terminal I/O through with_suspended_terminal ([30bb934](https://github.com/stevenwcarter/time-tracking-cli/commits/30bb934102376a478014d929e503ebc752b951a1))
* **tui:** stop the day and week list panes rendering blank ([170c759](https://github.com/stevenwcarter/time-tracking-cli/commits/170c759ea694d34f8fc170e70cf053bc8718c021))
* **tui:** stop the empty day pane from guessing from a stale load ([6ca858f](https://github.com/stevenwcarter/time-tracking-cli/commits/6ca858f250eba8d8b4fda2a26227b4eaa0658974))
* type parsedData with ParsedDayData interface (Q2) and format hours with .toFixed(2) (C9) ([14f1d8b](https://github.com/stevenwcarter/time-tracking-cli/commits/14f1d8bc09343c9ca9fa098457bf6d35f1ac2d33))
* use config week_start_day fallback in query-param week handler (C4 follow-up) ([169bfe3](https://github.com/stevenwcarter/time-tracking-cli/commits/169bfe3dede904ec12bc504a9498a8524dc38abd))
* use config week_start_day in path handler, sort project_summaries, use tracing ([89b3fef](https://github.com/stevenwcarter/time-tracking-cli/commits/89b3feff615d7b4e55edb556baeb4d944e85c104))
* **weekly-summary:** skip the clipboard copy and toast when a day has no notes ([bb38956](https://github.com/stevenwcarter/time-tracking-cli/commits/bb38956e5f06a065a05f633b3840f09cd0f203b7))


### Performance

* cache parsed day data in DataService ([b67debf](https://github.com/stevenwcarter/time-tracking-cli/commits/b67debfb9d1d9c147f5f196cc4cd201a5f16bf78))
* list the data directory once instead of probing 90 dates per keypress ([2496891](https://github.com/stevenwcarter/time-tracking-cli/commits/249689100c0eff7a9bb9965af74491a72d72a00d))
* **tui:** cache wrapped project-list bodies per width ([ef15d8d](https://github.com/stevenwcarter/time-tracking-cli/commits/ef15d8d44a0c7d8def53b58bad00b3b4d8b6075b))
* **tui:** derive week dates once per load instead of every frame ([480e386](https://github.com/stevenwcarter/time-tracking-cli/commits/480e3863a46918ab700541a055488dc863b26f16))
* **tui:** move data loads off the event loop with a latest-wins guard ([3afc002](https://github.com/stevenwcarter/time-tracking-cli/commits/3afc002ff34d8bd1d609d73ab111a785fde4da59))
* **tui:** redraw on state change instead of unconditionally at 30fps ([e660999](https://github.com/stevenwcarter/time-tracking-cli/commits/e66099942ca7a224791a79a9de37b4dd8dd87140))


### Style

* apply cargo fmt --all to previously unformatted files ([83f8d4d](https://github.com/stevenwcarter/time-tracking-cli/commits/83f8d4de7955712dd65d7d9f87f60d79379df02a))
* **tui:** rustfmt the bar chart hit-test assertion ([dbe142c](https://github.com/stevenwcarter/time-tracking-cli/commits/dbe142c6edae8b24db0cb71d36382d0b2df05b94))


### Docs

* add a runnable feature-combo gate recipe to Task 27 ([6fcd62e](https://github.com/stevenwcarter/time-tracking-cli/commits/6fcd62e15878c92e8ad4f17952b47e58f1c3f8e1))
* add a structural guard for EventHandler::start to Task 6 ([6eb8c1d](https://github.com/stevenwcarter/time-tracking-cli/commits/6eb8c1de863c9c564c5570b874967316af1bd8c4))
* add code health audit spec (14 correctness/observability fixes) ([84f5565](https://github.com/stevenwcarter/time-tracking-cli/commits/84f556587fa52c4bf4153bdff9a08f65e11ca401))
* add implementation plan for 14 code health fixes ([0617720](https://github.com/stevenwcarter/time-tracking-cli/commits/0617720b1aeca02164a5c5de9bf7bd4257683375))
* add the code-health execution spec and plan for the 2026-08-30 batch ([3d5bf11](https://github.com/stevenwcarter/time-tracking-cli/commits/3d5bf112ba83f1eda81fc6c89d88f6752f529ca5))
* add TUI overhaul design spec for the 26-item whats-next bundle ([12b42d7](https://github.com/stevenwcarter/time-tracking-cli/commits/12b42d795c836df72bcefa7930f249d70734bef0))
* add TUI overhaul implementation plan (27 tasks, 3 phases) ([847005e](https://github.com/stevenwcarter/time-tracking-cli/commits/847005e438b67dddd677699317cefdad24a4d226))
* apply doc-comment updates ([3d440db](https://github.com/stevenwcarter/time-tracking-cli/commits/3d440dbcd44fb5bc7ca8f6daef241921e818c4b4))
* apply pre-flight rulings R1 and R2 to the TUI overhaul plan ([bcf6805](https://github.com/stevenwcarter/time-tracking-cli/commits/bcf6805553b001c48e78bb2690fe5fab5dbae261))
* carry Task 15's leap-year clamp test into Task 27 ([62014f0](https://github.com/stevenwcarter/time-tracking-cli/commits/62014f005ff1cdd4c4b7b48d7a47096f22b007f8))
* carry Task 17's rejected-date assertion into Task 27 ([1ea45ca](https://github.com/stevenwcarter/time-tracking-cli/commits/1ea45ca526965115d47b510659dfdf14cfadcf92))
* carry Task 24's boundary-adjacent test into Task 27 ([39c663c](https://github.com/stevenwcarter/time-tracking-cli/commits/39c663c205bf90c6875708f320aa836e70a3f1f8))
* carry Task 4 review findings into Tasks 5, 6 and 17 ([cf79e0a](https://github.com/stevenwcarter/time-tracking-cli/commits/cf79e0a90260ac1e38af4dddd6b019fee9048bdf))
* carry Task 6 review findings into Tasks 7 and 12 ([11bf9af](https://github.com/stevenwcarter/time-tracking-cli/commits/11bf9aff440fdd00a587f657bc87d98aea7c1dfd))
* carry Task 6's startup no-data flash into Task 12 ([56c8215](https://github.com/stevenwcarter/time-tracking-cli/commits/56c82154f52a1f831e2b5cc3d3ff7055a33ec541))
* carry Task 7's tick(&self) constraint into Task 12 ([2d84a2b](https://github.com/stevenwcarter/time-tracking-cli/commits/2d84a2b7892bc64ff21b972999a10df6ae823c75))
* correct the breaking-change entry and refresh the stale docs ([dac7fa8](https://github.com/stevenwcarter/time-tracking-cli/commits/dac7fa852a9244ee0acf11578f761178ac5ac06b))
* correct the rustfmt.toml constraint — this repo has none ([512d7d1](https://github.com/stevenwcarter/time-tracking-cli/commits/512d7d1eaff25297bac47ee6390b96256738b815))
* document mouse support and Ctrl-Z suspend ([a3bf235](https://github.com/stevenwcarter/time-tracking-cli/commits/a3bf23523933f3683caf963452b009f972c64b11))
* fix three stale comments/changelog entries from triage ([bab4001](https://github.com/stevenwcarter/time-tracking-cli/commits/bab400114790062e4bb6ecf25052db363b417468))
* **plan:** implementation plan for TUI mouse support and Ctrl-Z suspend ([93317cc](https://github.com/stevenwcarter/time-tracking-cli/commits/93317ccf232de44b7cacd01814d6431dcd790d32))
* record weekly tie-ordering nondeterminism found while planning W18 ([1133e35](https://github.com/stevenwcarter/time-tracking-cli/commits/1133e3501dd73b0fdb6f9fc4e58d6fc90b1e64ae))
* refresh CLAUDE.md's tui/ module table and README config/feature docs ([9b77de5](https://github.com/stevenwcarter/time-tracking-cli/commits/9b77de51f16da64e302175b3d8e34d5a1019511a))
* require tokio::test for App-constructing tests across the plan ([a4a2182](https://github.com/stevenwcarter/time-tracking-cli/commits/a4a2182873dd7b5f90daec69948dd75b62719df9))
* **spec:** TUI mouse support and Ctrl-Z suspend (W29, W30) ([583aa62](https://github.com/stevenwcarter/time-tracking-cli/commits/583aa62e3ddbb2d9212da458609524e6af1f5e58))
* **tidy:** add execution spec for the 25-item batch ([2094e44](https://github.com/stevenwcarter/time-tracking-cli/commits/2094e44974c10bb81f6356c5c1b334268ccd07db))
* **tidy:** add implementation plan for the 25-item batch ([6679df0](https://github.com/stevenwcarter/time-tracking-cli/commits/6679df083f030b9e4a7f979d2fa2130123632bd7))
* **tui:** resolve help popup's quit/close contradiction, add README Mode column ([42b4fdf](https://github.com/stevenwcarter/time-tracking-cli/commits/42b4fdf12777071c9b9df428aa4a271403bf8332))
* warn Task 11 that the weekly warning filter is dead code ([743e657](https://github.com/stevenwcarter/time-tracking-cli/commits/743e6576db11f879371ec2651ebddca470cf4c76))


### Chore

* add a justfile gate recipe for the full verification matrix ([5f3b4fd](https://github.com/stevenwcarter/time-tracking-cli/commits/5f3b4fd0deb795c606822b18813f6dde30201c87))
* adding CLAUDE context file [skip ci] ([ee1984e](https://github.com/stevenwcarter/time-tracking-cli/commits/ee1984e369ecd06944d94624508e2bd4769e74b7))
* remove stale sample data comment from WeeklySummaryPage (Q1) ([d0a7aae](https://github.com/stevenwcarter/time-tracking-cli/commits/d0a7aaec8aa575e49aac72a4eb72d3fc04b71b87))
* small code clean-up, added helper trait ([118cddd](https://github.com/stevenwcarter/time-tracking-cli/commits/118cddddfd4bcb8dc5f1ba8980dd827c6bc0a61c))
* **tidy:** file T22 into the skip archive ([dd9be0e](https://github.com/stevenwcarter/time-tracking-cli/commits/dd9be0e0208db691844db022eac5d040d6cba3dd))
* **tidy:** record T50, a spurious empty save found during T18 ([2ca78c3](https://github.com/stevenwcarter/time-tracking-cli/commits/2ca78c3107ab9eba2e8de0fca07669f6972e142a))
* **tidy:** record triage findings ([99cbbe0](https://github.com/stevenwcarter/time-tracking-cli/commits/99cbbe086ca0130e6d3b14810571c5a995807c4c))
* v0.9.0 release ([0f9ab1b](https://github.com/stevenwcarter/time-tracking-cli/commits/0f9ab1bef5ab2c22cbaa721fdedc019388697e07))
* **whats-next:** file skips, mark W29/W30 in-flight for ship-it ([816020d](https://github.com/stevenwcarter/time-tracking-cli/commits/816020d56f59e1e87462efbd6d7f7cde49ac8859))
* **whats-next:** fold W10, W11, W17, W18, W28 into the in-flight bundle ([103c4f2](https://github.com/stevenwcarter/time-tracking-cli/commits/103c4f297d19d8486e7d8ae714f049c368325b99))
* **whats-next:** mark 21 selected items in-flight ([8162caa](https://github.com/stevenwcarter/time-tracking-cli/commits/8162caaba70923a4a25174346de45783f6c9c1e5))
* **whats-next:** strip the 26 items shipped by this branch ([c365f3f](https://github.com/stevenwcarter/time-tracking-cli/commits/c365f3f25aa3d3d8bccdc326a9d5994feb3e35e3))
* **whats-next:** strip W29 and W30, shipped by this merge ([988c92f](https://github.com/stevenwcarter/time-tracking-cli/commits/988c92f764a91e708e22d8f38828521b9a258e41))
* **whats-next:** triage findings ([b30f773](https://github.com/stevenwcarter/time-tracking-cli/commits/b30f7731eee5153a11dad2d6c9b29407ed2df445))

## [0.8.0](https://github.com/stevenwcarter/time-tracking-cli/compare/v0.9.0...v0.8.0) (2026-09-02)

## [0.8.0](https://github.com/stevenwcarter/time-tracking-cli/compare/v0.9.0...v0.8.0) (2026-09-02)

## [0.8.0](https://github.com/stevenwcarter/time-tracking-cli/compare/v0.7.0...v0.8.0) (2025-11-26)


### Features

* added lib capability to not parse args, such as when used from another library ([8d16033](https://github.com/stevenwcarter/time-tracking-cli/commits/8d160333f3d7fc51c28206c45418617ba2189f44))


### Style

* highlighted labels on bar charts ([1fcb04d](https://github.com/stevenwcarter/time-tracking-cli/commits/1fcb04da2f96ba810c4ab173c8ad5893bda7b24c))
* removing some backgrounds for better use of terminal transparency ([e057b0d](https://github.com/stevenwcarter/time-tracking-cli/commits/e057b0db57f776a1f8db74a39c05ecd25e5b9966))


### Docs

* updated README to add more details around TUI ([ee9d4d8](https://github.com/stevenwcarter/time-tracking-cli/commits/ee9d4d8fca76ac179e3405817eb90cd715e9e8ea))


### Performance

* enabled central DataService with caching, more ergonomic to use now ([e4ed8be](https://github.com/stevenwcarter/time-tracking-cli/commits/e4ed8bea6bd4e4c22f5541be8f719484a776d832))
* increased async await points to limit blocking ([5e7df7e](https://github.com/stevenwcarter/time-tracking-cli/commits/5e7df7e00aab82228f02ca65334e19fd9ee85fe4))


### Chore

* remove clap dep from non-cli builds as library usage ([9f0f6bf](https://github.com/stevenwcarter/time-tracking-cli/commits/9f0f6bfb015cafa64ae8011961ec952b5e3af7a5))
* removing default dependencies and adding build for intermediate commits and PRs ([d6af942](https://github.com/stevenwcarter/time-tracking-cli/commits/d6af94270b6939c37273314f089078984aa6ce0f))
* separating crate into lib/bin crates (cli) ([4f89608](https://github.com/stevenwcarter/time-tracking-cli/commits/4f89608fd4253de485506b6ce1789b3e0a2fccec))
* skip build.rs steps if webapp feature is not enabled [skip ci] ([518785b](https://github.com/stevenwcarter/time-tracking-cli/commits/518785bda9e7fb14d437dd0cfd601f89f35c46a4))
* v0.8.0 release ([96643da](https://github.com/stevenwcarter/time-tracking-cli/commits/96643da866dad0cadd627548ad236b3569448fb5))

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
