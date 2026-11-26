# Changelog

## [0.3.0](https://github.com/KinjiKawaguchi/agency/compare/v0.2.0...v0.3.0) (2026-11-26)


### Features

* **apple:** add a Mac app with a sidebar, sign-in and a Secure Enclave key ([#276](https://github.com/KinjiKawaguchi/agency/issues/276)) ([b0c7245](https://github.com/KinjiKawaguchi/agency/commit/b0c72450f12d3d400f9eb987f1023b5eff573e23))
* **apple:** add what a widget shows of the desk, and how it is written and read ([#260](https://github.com/KinjiKawaguchi/agency/issues/260)) ([1331c3e](https://github.com/KinjiKawaguchi/agency/commit/1331c3ed178e7012e77c0d3b318143596b9b683c))
* **apple:** choose which news is told, and register the kinds with the node ([#235](https://github.com/KinjiKawaguchi/agency/issues/235)) ([e729319](https://github.com/KinjiKawaguchi/agency/commit/e729319a58f9ae6570b340beeed914707fd73b59))
* **apple:** put the desk on the home screen and the lock screen as widgets ([#275](https://github.com/KinjiKawaguchi/agency/issues/275)) ([e1262db](https://github.com/KinjiKawaguchi/agency/commit/e1262dbf49bd3d098095fb20887d6840e8c4fe3e))
* **apple:** show the evaluation and where effort goes, in a results tab ([#229](https://github.com/KinjiKawaguchi/agency/issues/229)) ([92546b4](https://github.com/KinjiKawaguchi/agency/commit/92546b47e40e65f423d7a48bb96ebd2d95c6456b))
* **apple:** show the org chart of every org, in an organization tab ([#239](https://github.com/KinjiKawaguchi/agency/issues/239)) ([01b80e2](https://github.com/KinjiKawaguchi/agency/commit/01b80e2de3e68ad7776c9837308e21c74ac72426))
* **core:** compute how a KPI stood around the day an epic shipped ([#321](https://github.com/KinjiKawaguchi/agency/issues/321)) ([e6d3b51](https://github.com/KinjiKawaguchi/agency/commit/e6d3b51a77b3cc90ae6ff2a01b19f867791e88b6)), closes [#206](https://github.com/KinjiKawaguchi/agency/issues/206) [#320](https://github.com/KinjiKawaguchi/agency/issues/320)
* **node:** keep the org's numbers on the owner's node and serve them to the owner only ([#311](https://github.com/KinjiKawaguchi/agency/issues/311)) ([1b0485d](https://github.com/KinjiKawaguchi/agency/commit/1b0485dcf49e0bd7edce73470ecd67916003c566)), closes [#310](https://github.com/KinjiKawaguchi/agency/issues/310)
* **node:** notify the owner's browsers through Web Push ([#278](https://github.com/KinjiKawaguchi/agency/issues/278)) ([ec39c86](https://github.com/KinjiKawaguchi/agency/commit/ec39c866312383ce7664f0f998728c18e49f8af0))
* **node:** say in funding list which logins are past their threshold, blocked, or unknown ([#280](https://github.com/KinjiKawaguchi/agency/issues/280)) ([2c1826b](https://github.com/KinjiKawaguchi/agency/commit/2c1826b56c93f4d6f5a17e7fdb0130539db83d23))
* **node:** say in GET /api/me that the node hides titles ([#289](https://github.com/KinjiKawaguchi/agency/issues/289)) ([0dd7094](https://github.com/KinjiKawaguchi/agency/commit/0dd7094b3e79d1f2a5a4888ea395cf2dcf721240))
* **node:** serve a hosted team's delivery evaluation over the federation route ([#244](https://github.com/KinjiKawaguchi/agency/issues/244)) ([5be794f](https://github.com/KinjiKawaguchi/agency/commit/5be794f6ecdf65558c0df009349b49bf0b714bc6))
* **node:** serve every team's delivery evaluation of an org to its clients ([#291](https://github.com/KinjiKawaguchi/agency/issues/291)) ([25fc42d](https://github.com/KinjiKawaguchi/agency/commit/25fc42d295e0721d84c874043faae323cf363835))
* **node:** wake the iPhones to read the desk, so the widgets follow while the app is closed ([#305](https://github.com/KinjiKawaguchi/agency/issues/305)) ([61d2aa1](https://github.com/KinjiKawaguchi/agency/commit/61d2aa1ffa8dc5705d23407b392b74db5ac32645))
* **ops:** release the Apple apps as one artifact, tagged apple-v ([#220](https://github.com/KinjiKawaguchi/agency/issues/220)) ([be322cd](https://github.com/KinjiKawaguchi/agency/commit/be322cdf22dfadf3a7f2858ca6c4f69dbd192a8a))
* **proto:** add the types and the spec for the numbers of the outcome analysis ([#296](https://github.com/KinjiKawaguchi/agency/issues/296)) ([7bd9815](https://github.com/KinjiKawaguchi/agency/commit/7bd981583bc71e5bdb435c8a1ed7a1d11708253c)), closes [#293](https://github.com/KinjiKawaguchi/agency/issues/293)
* **proto:** state a team's plan in the org definition ([#154](https://github.com/KinjiKawaguchi/agency/issues/154)) ([1ede55a](https://github.com/KinjiKawaguchi/agency/commit/1ede55ac3853a2031b24aa2b83501758e6f90c1e))
* **proto:** state how a KPI moved after the epic an aim names shipped ([#316](https://github.com/KinjiKawaguchi/agency/issues/316)) ([6a27db9](https://github.com/KinjiKawaguchi/agency/commit/6a27db9cbd94bf4159e60712633a7008e9533239)), closes [#206](https://github.com/KinjiKawaguchi/agency/issues/206) [#312](https://github.com/KinjiKawaguchi/agency/issues/312)
* **proto:** state what the org measures and what each team aims at ([#287](https://github.com/KinjiKawaguchi/agency/issues/287)) ([41a7205](https://github.com/KinjiKawaguchi/agency/commit/41a72054dec9eeedeffe296ed019902c49e230ce))
* **web:** ask the browser to keep the device key, and say when it is gone ([#295](https://github.com/KinjiKawaguchi/agency/issues/295)) ([ddc5043](https://github.com/KinjiKawaguchi/agency/commit/ddc5043f8452ee98204a6572b198eb0c309fe6b4))
* **web:** make each chart of the evaluation one tab stop, walked with the arrow keys ([#252](https://github.com/KinjiKawaguchi/agency/issues/252)) ([5fc41f6](https://github.com/KinjiKawaguchi/agency/commit/5fc41f69f3b1c6088449dbbe047643a6ac5aafe2))
* **web:** make the web client an installable app that keeps only its built files ([#264](https://github.com/KinjiKawaguchi/agency/issues/264)) ([acb48d6](https://github.com/KinjiKawaguchi/agency/commit/acb48d652dab0a8ab53e66a07ed367d48f020c6e))
* **web:** mark an epic that has been finished, and put it after the open ones ([#232](https://github.com/KinjiKawaguchi/agency/issues/232)) ([08a2cbf](https://github.com/KinjiKawaguchi/agency/commit/08a2cbf4aa9a3208fce00e5a091c83cd546dce69))
* **web:** tell the viewer what is new through the operating system while the page is hidden ([#281](https://github.com/KinjiKawaguchi/agency/issues/281)) ([238c0d0](https://github.com/KinjiKawaguchi/agency/commit/238c0d0fb1e1bfbe85bcd2524ca58480ca476ca2))


### Bug fixes

* **apple:** show that the node has not answered yet, never a value in its place ([#307](https://github.com/KinjiKawaguchi/agency/issues/307)) ([649da24](https://github.com/KinjiKawaguchi/agency/commit/649da24dbba3b27ae3f90bdc299ab6863d31d69c))
* **deps:** update jsonwebtoken to 10, with the aws-lc-rs backend ([#297](https://github.com/KinjiKawaguchi/agency/issues/297)) ([2c6ad54](https://github.com/KinjiKawaguchi/agency/commit/2c6ad543f5be96be00d312e19ee00bc70f6152c6))
* **node:** answer 404 for a missing file of the web client, not the page ([#309](https://github.com/KinjiKawaguchi/agency/issues/309)) ([36c4857](https://github.com/KinjiKawaguchi/agency/commit/36c48570dbee39ec9e59e9854caf2c272dcd736c))
* **node:** refuse a push endpoint, a host list, and a browser key that cannot be right ([#294](https://github.com/KinjiKawaguchi/agency/issues/294)) ([7140e6a](https://github.com/KinjiKawaguchi/agency/commit/7140e6a80f98369e19ebaecf1c3fd6c8178d1356))
* **ops:** find the built binary where cargo put it ([#250](https://github.com/KinjiKawaguchi/agency/issues/250)) ([d5e2c9d](https://github.com/KinjiKawaguchi/agency/commit/d5e2c9de1b1800168eca901fc7f4253df1739049))
* **web:** renew the write token once when the node started again, and pin the Decide flows by tests ([#253](https://github.com/KinjiKawaguchi/agency/issues/253)) ([166048b](https://github.com/KinjiKawaguchi/agency/commit/166048bb7ee3ec5e628d0d8bb0c2e0bd03b2bce5))
* **web:** say the participants who wait as the phone does ([#254](https://github.com/KinjiKawaguchi/agency/issues/254)) ([fc667fd](https://github.com/KinjiKawaguchi/agency/commit/fc667fd112486933f054655c396b81e8c356fdff))
* **web:** say the sign-in expired only when the node, asked again, answers nothing ([#306](https://github.com/KinjiKawaguchi/agency/issues/306)) ([eafd85b](https://github.com/KinjiKawaguchi/agency/commit/eafd85bed4f8f6794ec5f5a270c8ae067769717f))
* **web:** word what is new as the iPhone banner does, with the reason a work item is blocked ([#241](https://github.com/KinjiKawaguchi/agency/issues/241)) ([640e74f](https://github.com/KinjiKawaguchi/agency/commit/640e74f834d6303436b2d9556f93fc263e94e2aa))


### Performance

* **adapters:** serve views and activity from memory, refreshed in the background ([#258](https://github.com/KinjiKawaguchi/agency/issues/258)) ([b1ba49e](https://github.com/KinjiKawaguchi/agency/commit/b1ba49e6e25a17ec3604148055375a080e086cd7))
* **adapters:** share a team's activity read between readers ([#246](https://github.com/KinjiKawaguchi/agency/issues/246)) ([fc8a1bc](https://github.com/KinjiKawaguchi/agency/commit/fc8a1bce00d9997de70c0153003cd82686e755e1)), closes [#245](https://github.com/KinjiKawaguchi/agency/issues/245)


### Refactoring

* **apple:** share the app code between the iPhone and the Mac ([#268](https://github.com/KinjiKawaguchi/agency/issues/268)) ([e1c3ebb](https://github.com/KinjiKawaguchi/agency/commit/e1c3ebbef99f8d2322ce2db9fbfb46b3f4924078))
* **core:** build a HostedTeam in one place ([#238](https://github.com/KinjiKawaguchi/agency/issues/238)) ([68f3ae8](https://github.com/KinjiKawaguchi/agency/commit/68f3ae86af8a038cbc6848e8f4b12129045d23df)), closes [#237](https://github.com/KinjiKawaguchi/agency/issues/237)
* **core:** build a NodeConfig through its constructor ([#301](https://github.com/KinjiKawaguchi/agency/issues/301)) ([8df52e4](https://github.com/KinjiKawaguchi/agency/commit/8df52e46ca9437c35c93fd0366a5018812462f0f))
* **core:** keep a team's delivery repositories with the team ([#271](https://github.com/KinjiKawaguchi/agency/issues/271)) ([f0e058a](https://github.com/KinjiKawaguchi/agency/commit/f0e058a3ff21b33688172e593c9a92181f4fed60))
* **proto:** build a TeamPlace in one place ([#283](https://github.com/KinjiKawaguchi/agency/issues/283)) ([e3f6c7a](https://github.com/KinjiKawaguchi/agency/commit/e3f6c7aa4f4b163f4a1f14acf621cd43eaa8e9fe)), closes [#282](https://github.com/KinjiKawaguchi/agency/issues/282)
* **web:** read the live feed as the generated types, and fix what the live node's feed showed ([#234](https://github.com/KinjiKawaguchi/agency/issues/234)) ([0d26458](https://github.com/KinjiKawaguchi/agency/commit/0d264580cc9e558bd687fd5bf8c5a36ae6962694))


### Documentation

* never show a placeholder value for something the node has not answered ([#304](https://github.com/KinjiKawaguchi/agency/issues/304)) ([c078171](https://github.com/KinjiKawaguchi/agency/commit/c078171f25476ea277e592378d4ee57fc835413a))
* say the PATH of the service must have every command the runtime runs, and log when one is missing ([#286](https://github.com/KinjiKawaguchi/agency/issues/286)) ([dd523b1](https://github.com/KinjiKawaguchi/agency/commit/dd523b1140c9f461568095bea057c23f1b4b2587))
* state that config and definition structs are built through their constructors ([#288](https://github.com/KinjiKawaguchi/agency/issues/288)) ([47a5d3b](https://github.com/KinjiKawaguchi/agency/commit/47a5d3be87b15cccc65665f6496633551e9a5463))
* state what each part may change without breaking the others ([#313](https://github.com/KinjiKawaguchi/agency/issues/313)) ([f071a06](https://github.com/KinjiKawaguchi/agency/commit/f071a0675efd4bc0efb6047cc8dceb982b24d304))

## [0.2.0](https://github.com/KinjiKawaguchi/agency/compare/v0.1.0...v0.2.0) (2026-11-26)


### Features

* **adapters:** read a team's delivery facts from GitHub through gh ([#233](https://github.com/KinjiKawaguchi/agency/issues/233)) ([d700b95](https://github.com/KinjiKawaguchi/agency/commit/d700b956c931455ed813ab9ce407bc3c481e1c85))
* **apple:** add the shared definition of what is new, and the engine for it ([#187](https://github.com/KinjiKawaguchi/agency/issues/187)) ([cdf7a4e](https://github.com/KinjiKawaguchi/agency/commit/cdf7a4efee655d20e1e94b4a75ca76e27020ed3c))
* **apple:** read the account and the teams once for all the screens ([#215](https://github.com/KinjiKawaguchi/agency/issues/215)) ([6770a1e](https://github.com/KinjiKawaguchi/agency/commit/6770a1ec50334e070db76533ea6c40ebb27ed76d))
* **apple:** show what is new as banners while the app is open ([#230](https://github.com/KinjiKawaguchi/agency/issues/230)) ([8c598f9](https://github.com/KinjiKawaguchi/agency/commit/8c598f99bd77b9d9904a75ad7de3cc6d38a65d6f))
* **apple:** wait and ask again when a team's data is not ready, not an error ([#188](https://github.com/KinjiKawaguchi/agency/issues/188)) ([59aa1bf](https://github.com/KinjiKawaguchi/agency/commit/59aa1bfefbc17077b02bf5c7b0622d12f0a0bd93))
* **core:** work out a team's delivery numbers from facts ([#216](https://github.com/KinjiKawaguchi/agency/issues/216)) ([577af58](https://github.com/KinjiKawaguchi/agency/commit/577af589ad978540b4dcc12914247ac24afdce5d))
* **node:** push a device request and a refused decision, for the kinds a phone wants ([#211](https://github.com/KinjiKawaguchi/agency/issues/211)) ([2646dd5](https://github.com/KinjiKawaguchi/agency/commit/2646dd50797fb1d85a2960d6354049317bc10438)), closes [#189](https://github.com/KinjiKawaguchi/agency/issues/189)
* **node:** serve a team's activity feed from its runtime ([#222](https://github.com/KinjiKawaguchi/agency/issues/222)) ([40e44b4](https://github.com/KinjiKawaguchi/agency/commit/40e44b40510d85d595f783c1f8bc2cb187eb9a94)), closes [#219](https://github.com/KinjiKawaguchi/agency/issues/219)
* **proto:** carry when a work item or an epic reached done ([#226](https://github.com/KinjiKawaguchi/agency/issues/226)) ([040fb3b](https://github.com/KinjiKawaguchi/agency/commit/040fb3ba64c552a50a788efabed88b4f61ba1bd6))
* **web:** add a live page to leave open, showing the department moving (experimental) ([#221](https://github.com/KinjiKawaguchi/agency/issues/221)) ([584f6d0](https://github.com/KinjiKawaguchi/agency/commit/584f6d0db2da9e54547bfd716d25858cf9df7dc8))


### Bug fixes

* **apple:** show a new device request on the devices screen without a reload ([#173](https://github.com/KinjiKawaguchi/agency/issues/173)) ([6791636](https://github.com/KinjiKawaguchi/agency/commit/6791636ad278d535cf6a1cda9e197e54ae4d4fd2))
* **core:** let a node that missed org versions take them in order ([#186](https://github.com/KinjiKawaguchi/agency/issues/186)) ([567aec9](https://github.com/KinjiKawaguchi/agency/commit/567aec99af34981a4a1a0ae85b2db343309874e4)), closes [#185](https://github.com/KinjiKawaguchi/agency/issues/185)
* **core:** make a removed team stop being served and kept ([#218](https://github.com/KinjiKawaguchi/agency/issues/218)) ([cb7f1bc](https://github.com/KinjiKawaguchi/agency/commit/cb7f1bcc8531d60e54c236d85cd8fc34d0cfd0c3)), closes [#180](https://github.com/KinjiKawaguchi/agency/issues/180)
* **web:** retry a failed region quietly, and let a page announce its loading once ([#213](https://github.com/KinjiKawaguchi/agency/issues/213)) ([780fa2e](https://github.com/KinjiKawaguchi/agency/commit/780fa2e20304d6bd1ced12411088ab47bb884acb))


### Refactoring

* **apple:** remove the web view and everything only it used ([#192](https://github.com/KinjiKawaguchi/agency/issues/192)) ([92a1a16](https://github.com/KinjiKawaguchi/agency/commit/92a1a16044296126bd8d86a30385a52c6a9c74ec))
* **web:** read the evaluation and effort answers as the generated types ([#212](https://github.com/KinjiKawaguchi/agency/issues/212)) ([ced6dc4](https://github.com/KinjiKawaguchi/agency/commit/ced6dc46a1a74af6662cb84e011541936663dabe))
* **web:** read the node's command for approving a device as the generated type ([#228](https://github.com/KinjiKawaguchi/agency/issues/228)) ([ee3cf7c](https://github.com/KinjiKawaguchi/agency/commit/ee3cf7cd8fb86c1c98d8902ea972d6abca9b5b1e))

## 0.1.0 (2026-11-26)


### Features

* **ops:** make releases with release-please, with the Linux builds attached by CI ([#205](https://github.com/KinjiKawaguchi/agency/issues/205)) ([2c0a482](https://github.com/KinjiKawaguchi/agency/commit/2c0a4824c87a74182952ff43ecd143655d6768a9))
* **web:** tell the viewer in the page what is new, and mark the avatar ([#202](https://github.com/KinjiKawaguchi/agency/issues/202)) ([41612ae](https://github.com/KinjiKawaguchi/agency/commit/41612aedebf226e37334ef1274eaf175c9f2124b))
