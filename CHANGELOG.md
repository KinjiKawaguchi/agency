# Changelog

## [0.4.0](https://github.com/KinjiKawaguchi/agency/compare/v0.3.0...v0.4.0) (2026-11-28)


### ⚠ BREAKING CHANGES

* **node:** the oidc setting names ownerSubject (the issuer's sub for the owner) in place of ownerEmail.

### Features

* **apple:** a wake that names a home node keeps that node's count true ([#404](https://github.com/KinjiKawaguchi/agency/issues/404)) ([46d90ad](https://github.com/KinjiKawaguchi/agency/commit/46d90adfc7f2034cd3aa335c4300fa7d797fb99a))
* **apple:** decide and move on at once, and keep sending in the background ([#471](https://github.com/KinjiKawaguchi/agency/issues/471)) ([c16f854](https://github.com/KinjiKawaguchi/agency/commit/c16f8541ca38018f71dcecc0214da15c74745a3a))
* **apple:** draw the node's p95 hour by hour ([#473](https://github.com/KinjiKawaguchi/agency/issues/473)) ([43625ac](https://github.com/KinjiKawaguchi/agency/commit/43625acf497f2608bc6c68225681623870f12fdb))
* **apple:** enter a KPI's number and read its history on the outcomes page ([#375](https://github.com/KinjiKawaguchi/agency/issues/375)) ([cbdb5dd](https://github.com/KinjiKawaguchi/agency/commit/cbdb5dd22830f421154f30d1a05d5ed0e953525f))
* **apple:** enter a KPI's number and read its history, without the screen ([#373](https://github.com/KinjiKawaguchi/agency/issues/373)) ([c38cb0d](https://github.com/KinjiKawaguchi/agency/commit/c38cb0da75b0f581edfacd186e8ded00710f0cc6))
* **apple:** give a member the node hosts their own screens, not the owner's tabs ([#427](https://github.com/KinjiKawaguchi/agency/issues/427)) ([60c2c48](https://github.com/KinjiKawaguchi/agency/commit/60c2c48ce16da4a707f57879ba57e27d32f1c0c5))
* **apple:** keep sending kept decisions after the app is left ([#479](https://github.com/KinjiKawaguchi/agency/issues/479)) ([af52bb4](https://github.com/KinjiKawaguchi/agency/commit/af52bb461147ff621980fa5b66c2ec1e8c56d13e))
* **apple:** name this phone's device in its push registration ([#466](https://github.com/KinjiKawaguchi/agency/issues/466)) ([866d80b](https://github.com/KinjiKawaguchi/agency/commit/866d80bef6473ab4b805e70fab2178b94d6c841d))
* **apple:** one number on the icon for all home nodes ([#401](https://github.com/KinjiKawaguchi/agency/issues/401)) ([6d36db7](https://github.com/KinjiKawaguchi/agency/commit/6d36db7b1cd3146aef98afbbc320ed6c2775d259))
* **apple:** open a tapped notification on the home node that sent it ([#383](https://github.com/KinjiKawaguchi/agency/issues/383)) ([bf6afd2](https://github.com/KinjiKawaguchi/agency/commit/bf6afd254dd227120b307dac7f7c22a5d40f0a37))
* **apple:** open the desk from the push for a held root change ([#398](https://github.com/KinjiKawaguchi/agency/issues/398)) ([7591b50](https://github.com/KinjiKawaguchi/agency/commit/7591b50a9eedc8126e2f358c91202a8f7e1bac2f))
* **apple:** read and word an objective the measurement cannot settle ([#424](https://github.com/KinjiKawaguchi/agency/issues/424)) ([6934e96](https://github.com/KinjiKawaguchi/agency/commit/6934e96f68625c1850f011cd2ab55e289f73367c))
* **apple:** read the desk when the node wakes the app, so the widgets follow while it is closed ([#315](https://github.com/KinjiKawaguchi/agency/issues/315)) ([c826197](https://github.com/KinjiKawaguchi/agency/commit/c82619762b7c5474b61389a9f4cddeab16c8cc30))
* **apple:** read the owner's outcome numbers and what moved after the work shipped ([#343](https://github.com/KinjiKawaguchi/agency/issues/343)) ([8bc9854](https://github.com/KinjiKawaguchi/agency/commit/8bc98544827390ff6595aa5f0ad1c15b43c7bb94))
* **apple:** remove one of the owner's devices from the Devices screen ([#459](https://github.com/KinjiKawaguchi/agency/issues/459)) ([a2d81a7](https://github.com/KinjiKawaguchi/agency/commit/a2d81a77a2355154c64000480841f6fc3b7b415e))
* **apple:** say a device approval the root's node holds, and show what waits for the root key ([#389](https://github.com/KinjiKawaguchi/agency/issues/389)) ([80afd56](https://github.com/KinjiKawaguchi/agency/commit/80afd56b35df39c86e2caac677a7b84421543450))
* **apple:** say on the Devices screen whether APNs takes this phone's notices ([#423](https://github.com/KinjiKawaguchi/agency/issues/423)) ([1ed232d](https://github.com/KinjiKawaguchi/agency/commit/1ed232df275212b642daf69da325f3c3f7ed4e28))
* **apple:** say which of the app and the node is to be updated ([#606](https://github.com/KinjiKawaguchi/agency/issues/606)) ([d02ab20](https://github.com/KinjiKawaguchi/agency/commit/d02ab20281a5bf22651cd9369c8456b323ce595a))
* **apple:** say whom a request is answered for, and show it in the decision log ([#408](https://github.com/KinjiKawaguchi/agency/issues/408)) ([24fc2e2](https://github.com/KinjiKawaguchi/agency/commit/24fc2e24f17183a4ad926e1cc50836bb52e09a6c))
* **apple:** show a label from the node without the characters that can reorder it ([#416](https://github.com/KinjiKawaguchi/agency/issues/416)) ([556fc9f](https://github.com/KinjiKawaguchi/agency/commit/556fc9f8e9d0eb757d4cf500f12d04311648ebda))
* **apple:** show a started recovery of an org at the top of the desk ([#369](https://github.com/KinjiKawaguchi/agency/issues/369)) ([b726239](https://github.com/KinjiKawaguchi/agency/commit/b726239f0c85cbcc233f29a9f2668224b32eb19f))
* **apple:** show the node's own response times at the end of the evaluation ([#403](https://github.com/KinjiKawaguchi/agency/issues/403)) ([7578f20](https://github.com/KinjiKawaguchi/agency/commit/7578f2074dbc4e39c85476e7aeb8c421941b7a3d))
* **apple:** show the owner's outcome numbers as a page of the results tab ([#348](https://github.com/KinjiKawaguchi/agency/issues/348)) ([443dbed](https://github.com/KinjiKawaguchi/agency/commit/443dbed6e31b8217405830861cf3e1933015545d))
* **apple:** show the text agents write without the characters that can reorder it ([#433](https://github.com/KinjiKawaguchi/agency/issues/433)) ([8709a81](https://github.com/KinjiKawaguchi/agency/commit/8709a8174b6681a4bee2c82a5306a19bd86b32ad))
* **apple:** show what each participant of a team did in its evaluation ([#380](https://github.com/KinjiKawaguchi/agency/issues/380)) ([33b3e23](https://github.com/KinjiKawaguchi/agency/commit/33b3e2384db7b10e122b74ebd20e6a4bcd8a1245))
* **apple:** show what the work cost on the same days under each KPI ([#411](https://github.com/KinjiKawaguchi/agency/issues/411)) ([0707553](https://github.com/KinjiKawaguchi/agency/commit/0707553e9ab59093f7aeadd133a96e40384a380f))
* **apple:** show what waits in the Mac's menu bar, and open settings on ⌘, ([#277](https://github.com/KinjiKawaguchi/agency/issues/277)) ([9aafada](https://github.com/KinjiKawaguchi/agency/commit/9aafada474fbd65918bcb2a25b092c08f0369f35))
* **apple:** switch between home nodes, and sign in to a node on any host ([#357](https://github.com/KinjiKawaguchi/agency/issues/357)) ([2826eb4](https://github.com/KinjiKawaguchi/agency/commit/2826eb455d93ee99f2926824bb74c212a265619f))
* **apple:** tell the node which app version is calling ([#352](https://github.com/KinjiKawaguchi/agency/issues/352)) ([76bd057](https://github.com/KinjiKawaguchi/agency/commit/76bd05728cbda2bebb23f17693a73c75de525b6d))
* **apple:** word a recovery's and a held root change's push in the viewer's language and zone ([#406](https://github.com/KinjiKawaguchi/agency/issues/406)) ([aa3f8c0](https://github.com/KinjiKawaguchi/agency/commit/aa3f8c07c6648d6e4b5c29ba7400861294dc5b0c))
* **core:** calculate weekly budget status ([#397](https://github.com/KinjiKawaguchi/agency/issues/397)) ([b80323f](https://github.com/KinjiKawaguchi/agency/commit/b80323f6622b11fef5d84c9932693b494680c6ff))
* **core:** hold back a version with a new field until every node lists it ([#350](https://github.com/KinjiKawaguchi/agency/issues/350)) ([d75b14d](https://github.com/KinjiKawaguchi/agency/commit/d75b14d9e5ab1af33b1e723541577d8dc4d58b6f))
* **core:** read a remote team's missing feature as not supported ([#602](https://github.com/KinjiKawaguchi/agency/issues/602)) ([8a3ba07](https://github.com/KinjiKawaguchi/agency/commit/8a3ba0776db8e4a8296ade0a0c8f11c417e0359b))
* **core:** redeem a claim link with a device key ([#548](https://github.com/KinjiKawaguchi/agency/issues/548)) ([df0410a](https://github.com/KinjiKawaguchi/agency/commit/df0410a799c79e58bdbe41a0ac5b1d3c4bd41745))
* **core:** remove a member from an org, with devices and sign-in ([#586](https://github.com/KinjiKawaguchi/agency/issues/586)) ([f10fd60](https://github.com/KinjiKawaguchi/agency/commit/f10fd6001dab8ea36a8cc3d7b4972b6f472301d3))
* **core:** turn a seat into a member the root's node hosts ([#525](https://github.com/KinjiKawaguchi/agency/issues/525)) ([81481da](https://github.com/KinjiKawaguchi/agency/commit/81481daaf8b2f3ca56f97d0858f1182d04db6ec4))
* **node:** add agency logs to read the node's log filtered by time, level, and event ([#342](https://github.com/KinjiKawaguchi/agency/issues/342)) ([594b4e1](https://github.com/KinjiKawaguchi/agency/commit/594b4e162dc1b20e9c99962c014c8bec10a80a2b))
* **node:** add agency metrics to read the node's own measurements as a table ([#414](https://github.com/KinjiKawaguchi/agency/issues/414)) ([c0725a1](https://github.com/KinjiKawaguchi/agency/commit/c0725a198be952e4e74ddf3e7ff6ff8dca5e85c4))
* **node:** admit members hosted by this node as guests, to their own requests only ([#430](https://github.com/KinjiKawaguchi/agency/issues/430)) ([f1add77](https://github.com/KinjiKawaguchi/agency/commit/f1add77a337e4122108f098db0b33120805cef63))
* **node:** answer a decision once it is kept and deliver it in the background ([#464](https://github.com/KinjiKawaguchi/agency/issues/464)) ([ac0cf72](https://github.com/KinjiKawaguchi/agency/commit/ac0cf72a9e345aa131f9db73b9253a3bba396bff))
* **node:** answer a seat's or an external party's request through the members named for it ([#379](https://github.com/KinjiKawaguchi/agency/issues/379)) ([3c10e84](https://github.com/KinjiKawaguchi/agency/commit/3c10e846ba1454a83ca1296041308d28bd7a6487))
* **node:** answer for a seat on a team that runs on another node ([#412](https://github.com/KinjiKawaguchi/agency/issues/412)) ([a3ec6df](https://github.com/KinjiKawaguchi/agency/commit/a3ec6df7440a5a5bea845c34343e9b338be2a00a))
* **node:** back off a team node that does not answer replica reads ([#555](https://github.com/KinjiKawaguchi/agency/issues/555)) ([987c41f](https://github.com/KinjiKawaguchi/agency/commit/987c41f07d4fea530450f60fcf299cd788f319fd))
* **node:** bound the org's history with a limit and a cursor ([#622](https://github.com/KinjiKawaguchi/agency/issues/622)) ([06cbf8c](https://github.com/KinjiKawaguchi/agency/commit/06cbf8c1c93728e324c0163c1742ddd0c7d4aa47))
* **node:** break a team's delivery numbers down by repository ([#605](https://github.com/KinjiKawaguchi/agency/issues/605)) ([d7f044c](https://github.com/KinjiKawaguchi/agency/commit/d7f044c622070dd1f7d2f5cd3ece8b14661c6665))
* **node:** call a runtime's participantMetrics, and name each pull request by repository and number ([#355](https://github.com/KinjiKawaguchi/agency/issues/355)) ([741a472](https://github.com/KinjiKawaguchi/agency/commit/741a472811ccd014ecb304550bd28748ff7063d8))
* **node:** confirm a pending claim at the node's terminal ([#554](https://github.com/KinjiKawaguchi/agency/issues/554)) ([91a177d](https://github.com/KinjiKawaguchi/agency/commit/91a177d25a1c3483886f781b2bd30ae4f6470243))
* **node:** confirm or refuse a pending claim from the owner's client ([#556](https://github.com/KinjiKawaguchi/agency/issues/556)) ([9376b6f](https://github.com/KinjiKawaguchi/agency/commit/9376b6fc586bd83fe266bacc8316c5344826924d))
* **node:** count a fix of an incident issue as incident effort ([#486](https://github.com/KinjiKawaguchi/agency/issues/486)) ([fe87957](https://github.com/KinjiKawaguchi/agency/commit/fe879577ca0214c441f6f566f8f38d16ebaf46a2))
* **node:** export an org's records, verifiable on their own ([#603](https://github.com/KinjiKawaguchi/agency/issues/603)) ([fd3a99c](https://github.com/KinjiKawaguchi/agency/commit/fd3a99c251be1792251f394570dff581af6e2e9f))
* **node:** give each org a root key of its own, and let the root rotate it ([#336](https://github.com/KinjiKawaguchi/agency/issues/336)) ([95e548d](https://github.com/KinjiKawaguchi/agency/commit/95e548d84d15d9f4db8b4e58a4a859afb9dacba8))
* **node:** give the buckets of each hour of a series when asked for them ([#429](https://github.com/KinjiKawaguchi/agency/issues/429)) ([3a69c4d](https://github.com/KinjiKawaguchi/agency/commit/3a69c4d98fca202d9748a10d88921fdcccf5b839)), closes [#240](https://github.com/KinjiKawaguchi/agency/issues/240)
* **node:** hand the app a single-use code at the sign-in, not the Access token ([#334](https://github.com/KinjiKawaguchi/agency/issues/334)) ([55908ef](https://github.com/KinjiKawaguchi/agency/commit/55908ef6bfb60c10070aa15c30004887ca6f5736))
* **node:** have Claude Code renew a login nobody uses, so its usage is not unknown for ever ([#349](https://github.com/KinjiKawaguchi/agency/issues/349)) ([22116d1](https://github.com/KinjiKawaguchi/agency/commit/22116d11290e17c3f923fda8f02ffb41a1a5c91b)), closes [#338](https://github.com/KinjiKawaguchi/agency/issues/338)
* **node:** hold a claim's confirmation from a client for the root key ([#557](https://github.com/KinjiKawaguchi/agency/issues/557)) ([4d536fb](https://github.com/KinjiKawaguchi/agency/commit/4d536fb313b2ace69b62b86c1fd55bba307f0195))
* **node:** hold root signatures others ask for until the owner signs them together ([#381](https://github.com/KinjiKawaguchi/agency/issues/381)) ([0a19a1a](https://github.com/KinjiKawaguchi/agency/commit/0a19a1a6f77f024dd7836e867a55717f91e9214d))
* **node:** keep root keys in the macOS keychain, signed only with the owner's presence ([#366](https://github.com/KinjiKawaguchi/agency/issues/366)) ([4274006](https://github.com/KinjiKawaguchi/agency/commit/427400621599294ad6854782be1642c3b0913fbb))
* **node:** keep when APNs last took a notice for each phone and when it last did not ([#409](https://github.com/KinjiKawaguchi/agency/issues/409)) ([3272761](https://github.com/KinjiKawaguchi/agency/commit/3272761c5e4fde15d4e22a466aee183bf7058003))
* **node:** keep when each browser's push service last took a notice and when it last did not ([#386](https://github.com/KinjiKawaguchi/agency/issues/386)) ([75c8d34](https://github.com/KinjiKawaguchi/agency/commit/75c8d34c385ed42fdbe47c44a693f70f61f00f27))
* **node:** let a guest's second browser ask to be added ([#528](https://github.com/KinjiKawaguchi/agency/issues/528)) ([1f774b4](https://github.com/KinjiKawaguchi/agency/commit/1f774b4e7d8d3a25b3afb579e74767319949ba11)), closes [#515](https://github.com/KinjiKawaguchi/agency/issues/515)
* **node:** let the operator set objectives for the node and say how the measurements stand against them ([#413](https://github.com/KinjiKawaguchi/agency/issues/413)) ([7aeb60c](https://github.com/KinjiKawaguchi/agency/commit/7aeb60cded1818d728560f590ae16e6574e32875))
* **node:** list an org's members, devices, and seats for its owner ([#585](https://github.com/KinjiKawaguchi/agency/issues/585)) ([946db3c](https://github.com/KinjiKawaguchi/agency/commit/946db3c964dbe2ceadd73fc18897bcf9efef3407))
* **node:** list who an org's agents may ask for what ([#390](https://github.com/KinjiKawaguchi/agency/issues/390)) ([5adf949](https://github.com/KinjiKawaguchi/agency/commit/5adf949b31089383942dd503a12eb88e3f3f5c5b))
* **node:** make seat claim links ([#524](https://github.com/KinjiKawaguchi/agency/issues/524)) ([acd3d8d](https://github.com/KinjiKawaguchi/agency/commit/acd3d8da117514cb57954457fd0b98ea1d9da1b9))
* **node:** name the node in every push and let the app change an alert's badge ([#377](https://github.com/KinjiKawaguchi/agency/issues/377)) ([dd4cc59](https://github.com/KinjiKawaguchi/agency/commit/dd4cc59e3ae3aad3594b5e221bee1bd24bb1298b))
* **node:** note which app versions call the node ([#358](https://github.com/KinjiKawaguchi/agency/issues/358)) ([5ea9809](https://github.com/KinjiKawaguchi/agency/commit/5ea9809d35d0e7bc955800bb07910612f4504772)), closes [#354](https://github.com/KinjiKawaguchi/agency/issues/354)
* **node:** notify a guest of its own requests ([#496](https://github.com/KinjiKawaguchi/agency/issues/496)) ([aab73ca](https://github.com/KinjiKawaguchi/agency/commit/aab73caa53d168500a531800295353701ffa7802))
* **node:** put each participant's time, cost, and pull requests in the evaluation ([#364](https://github.com/KinjiKawaguchi/agency/issues/364)) ([e2779e0](https://github.com/KinjiKawaguchi/agency/commit/e2779e0de5a677581869842f36711e6f39b6f065))
* **node:** read a team on another node's activity feed live ([#436](https://github.com/KinjiKawaguchi/agency/issues/436)) ([d776d13](https://github.com/KinjiKawaguchi/agency/commit/d776d136c3f0a53bfe194713f3a72079dbdc9b23))
* **node:** read a team's view only when it changed ([#560](https://github.com/KinjiKawaguchi/agency/issues/560)) ([db96e93](https://github.com/KinjiKawaguchi/agency/commit/db96e935bdbba07f4e698f0c06b0f3c14d7f50ff))
* **node:** read and keep what the org's other nodes speak ([#346](https://github.com/KinjiKawaguchi/agency/issues/346)) ([6a2864d](https://github.com/KinjiKawaguchi/agency/commit/6a2864dd370b0fb719e5231723354a28e1770e66))
* **node:** read delivery numbers from a team's own GitHub host ([#609](https://github.com/KinjiKawaguchi/agency/issues/609)) ([397e949](https://github.com/KinjiKawaguchi/agency/commit/397e9498710af0a912da470e1140b5b8f427fa39))
* **node:** read other nodes at once, apart from notify and deliver ([#553](https://github.com/KinjiKawaguchi/agency/issues/553)) ([015a1a4](https://github.com/KinjiKawaguchi/agency/commit/015a1a4d75d5e916574e2af316b23d68e27aa412))
* **node:** record the node's own measurements by hour and keep them in files ([#388](https://github.com/KinjiKawaguchi/agency/issues/388)) ([663c556](https://github.com/KinjiKawaguchi/agency/commit/663c5567140978c3e55fa1bdf8d21e3b0e249d54))
* **node:** record which device signed each decision in the decision log ([#503](https://github.com/KinjiKawaguchi/agency/issues/503)) ([dd86449](https://github.com/KinjiKawaguchi/agency/commit/dd86449b2d9d262a7eac9e0df601efc62257d9ed)), closes [#485](https://github.com/KinjiKawaguchi/agency/issues/485)
* **node:** refuse both or a bad sign-in setting at start ([#627](https://github.com/KinjiKawaguchi/agency/issues/627)) ([13d6045](https://github.com/KinjiKawaguchi/agency/commit/13d60454669926cec7fa5a10026808f66bcf85af))
* **node:** refuse to start on data or settings of a newer release ([#599](https://github.com/KinjiKawaguchi/agency/issues/599)) ([37c59ea](https://github.com/KinjiKawaguchi/agency/commit/37c59ead263ca89830d8d3c7eff5f3609e8f0767))
* **node:** remove hosted members ([#590](https://github.com/KinjiKawaguchi/agency/issues/590)) ([2ab7faf](https://github.com/KinjiKawaguchi/agency/commit/2ab7faf6db76536b80eb4b299dc0ec28613f96e9))
* **node:** report weekly budget status ([#439](https://github.com/KinjiKawaguchi/agency/issues/439)) ([f6955b6](https://github.com/KinjiKawaguchi/agency/commit/f6955b622d4d55ab03861c6fa38e345e9e362b69))
* **node:** serve a team's weekly budget status from the node ([#498](https://github.com/KinjiKawaguchi/agency/issues/498)) ([f1bc0c6](https://github.com/KinjiKawaguchi/agency/commit/f1bc0c643f1a6bf56a06a1e20cc74b4dfe8c577c))
* **node:** serve the claim routes to a claimant and to no one else ([#551](https://github.com/KinjiKawaguchi/agency/issues/551)) ([c34b40f](https://github.com/KinjiKawaguchi/agency/commit/c34b40ff89e04ed73cc5a09cb7b9e0a1ea959fea))
* **node:** serve the org's change history to its owner ([#616](https://github.com/KinjiKawaguchi/agency/issues/616)) ([818c7b6](https://github.com/KinjiKawaguchi/agency/commit/818c7b6a364ae6c6cacb347e4bb225d9363668ce))
* **node:** serve to the owner how each KPI moved after its epic shipped ([#329](https://github.com/KinjiKawaguchi/agency/issues/329)) ([6c8bb5f](https://github.com/KinjiKawaguchi/agency/commit/6c8bb5f31d68446a9c78dabe22dd519f2579c7b2)), closes [#206](https://github.com/KinjiKawaguchi/agency/issues/206) [#328](https://github.com/KinjiKawaguchi/agency/issues/328)
* **node:** serve where each team's tokens went by work type ([#484](https://github.com/KinjiKawaguchi/agency/issues/484)) ([6488beb](https://github.com/KinjiKawaguchi/agency/commit/6488beb4a993c4000132b3f84bb7b09e8278c2e4))
* **node:** show which release each team's runtime is in status ([#604](https://github.com/KinjiKawaguchi/agency/issues/604)) ([f4186ff](https://github.com/KinjiKawaguchi/agency/commit/f4186ff02938343ebaa7f50f701b6ad403278605))
* **node:** sign a read for the node it is made for ([#497](https://github.com/KinjiKawaguchi/agency/issues/497)) ([daf83ec](https://github.com/KinjiKawaguchi/agency/commit/daf83ecd883598d898b3ed46ab3c08f4d9957dca))
* **node:** split effort by the board's Work type when an issue has one ([#488](https://github.com/KinjiKawaguchi/agency/issues/488)) ([efd5f53](https://github.com/KinjiKawaguchi/agency/commit/efd5f539eb041fad815415cd8719f4be9bb9d7c4))
* **node:** take a seat with a claim link or a merge, and keep its id as formerly ([#405](https://github.com/KinjiKawaguchi/agency/issues/405)) ([3a306b1](https://github.com/KinjiKawaguchi/agency/commit/3a306b1f654bf59dc8155cafa33c648ae6496e69))
* **node:** take the latency a client observed of its own calls ([#450](https://github.com/KinjiKawaguchi/agency/issues/450)) ([d7a1a16](https://github.com/KinjiKawaguchi/agency/commit/d7a1a16796766913abc2dd12eff80713f655e18a)), closes [#240](https://github.com/KinjiKawaguchi/agency/issues/240)
* **node:** tell the owner of a started recovery, and recover an org with its words ([#360](https://github.com/KinjiKawaguchi/agency/issues/360)) ([1405316](https://github.com/KinjiKawaguchi/agency/commit/14053164f97a1cb3e66ca64614fe2a1d2c63cb13))
* **node:** tell the owner of each held root change, and keep hidden characters out of labels ([#391](https://github.com/KinjiKawaguchi/agency/issues/391)) ([f17033b](https://github.com/KinjiKawaguchi/agency/commit/f17033bb47e682c29389caab5a4cf524abb10d8e))
* **node:** tell the owner's clients where their other home nodes are ([#308](https://github.com/KinjiKawaguchi/agency/issues/308)) ([dfe1db7](https://github.com/KinjiKawaguchi/agency/commit/dfe1db7a517c905373b4904d58c3e872a88962aa))
* **node:** time every route and let the owner read the node's measurements ([#400](https://github.com/KinjiKawaguchi/agency/issues/400)) ([914610b](https://github.com/KinjiKawaguchi/agency/commit/914610b7b757d74f2062b551a24393c14999edc8))
* **node:** time the calls a node makes ([#552](https://github.com/KinjiKawaguchi/agency/issues/552)) ([e93e872](https://github.com/KinjiKawaguchi/agency/commit/e93e872119c7d674b0e435c380dd58ce11c9bba4))
* **node:** write the node's log as rotating JSON lines in a file of its own ([#323](https://github.com/KinjiKawaguchi/agency/issues/323)) ([b9e244a](https://github.com/KinjiKawaguchi/agency/commit/b9e244ad498b1ed1891dcd3bbee880c51f4bbbcc))
* **proto:** add the participantMetrics op for what GitHub does not know ([#332](https://github.com/KinjiKawaguchi/agency/issues/332)) ([6aff4b1](https://github.com/KinjiKawaguchi/agency/commit/6aff4b1caab36193099544e95b665a4fd3621d68))
* **proto:** add the types of the node's own measurements, for the owner to read ([#378](https://github.com/KinjiKawaguchi/agency/issues/378)) ([712f3d1](https://github.com/KinjiKawaguchi/agency/commit/712f3d1196909ab072846e3fc16bd582b4568b16))
* **proto:** let a node say which versions and capabilities it speaks ([#337](https://github.com/KinjiKawaguchi/agency/issues/337)) ([eb449b4](https://github.com/KinjiKawaguchi/agency/commit/eb449b4c612346f5428cf60763d9e7a1bb57e05d)), closes [#318](https://github.com/KinjiKawaguchi/agency/issues/318)
* **proto:** let a recovery key move the root after a 72-hour wait the root can cancel ([#353](https://github.com/KinjiKawaguchi/agency/issues/353)) ([acd49b4](https://github.com/KinjiKawaguchi/agency/commit/acd49b44966f79bb0f62de8aa1b39a17b018fbef))
* **proto:** print text that someone else wrote without what hides or reorders a line, and use it in the CLI ([#437](https://github.com/KinjiKawaguchi/agency/issues/437)) ([61c2829](https://github.com/KinjiKawaguchi/agency/commit/61c2829ae4348c77ef5fd3a38d7c7a2fb4b72e8b))
* **proto:** state members hosted by the root's node, who have no node and no account key ([#418](https://github.com/KinjiKawaguchi/agency/issues/418)) ([d46e0a7](https://github.com/KinjiKawaguchi/agency/commit/d46e0a7236b571c12d99f3043642210516f71d1e))
* **proto:** state seats and external parties in the org definition ([#371](https://github.com/KinjiKawaguchi/agency/issues/371)) ([8d4507a](https://github.com/KinjiKawaguchi/agency/commit/8d4507adaa86a29a6848e66b0cf6e3fd1e5c970f)), closes [#196](https://github.com/KinjiKawaguchi/agency/issues/196)
* **release:** package the node for Debian ([#285](https://github.com/KinjiKawaguchi/agency/issues/285)) ([efbd850](https://github.com/KinjiKawaguchi/agency/commit/efbd850a185b2622caca3c273eed958635864675))
* **web:** ask for a team's effort only of a node that says it serves it ([#596](https://github.com/KinjiKawaguchi/agency/issues/596)) ([09a21a1](https://github.com/KinjiKawaguchi/agency/commit/09a21a1518c095dfc0fc340260b8a8f514acfe60))
* **web:** claim a seat by link, against the mock node ([#347](https://github.com/KinjiKawaguchi/agency/issues/347)) ([63b5cac](https://github.com/KinjiKawaguchi/agency/commit/63b5cace03e94412364603fa03a0f0da7b8f4bf6))
* **web:** count only the undelivered decisions the device signed ([#564](https://github.com/KinjiKawaguchi/agency/issues/564)) ([975e0b8](https://github.com/KinjiKawaguchi/agency/commit/975e0b8356ef911239ac647e6d2371c5ef360cd2))
* **web:** decide and move on at once, and keep sending in the background ([#461](https://github.com/KinjiKawaguchi/agency/issues/461)) ([bd4a069](https://github.com/KinjiKawaguchi/agency/commit/bd4a069a0c9bdbd9948bd44c20174590f4f4863d))
* **web:** draw the node's p95 hour by hour, with the budget and the gaps ([#415](https://github.com/KinjiKawaguchi/agency/issues/415)) ([10ba934](https://github.com/KinjiKawaguchi/agency/commit/10ba93498fe7ce729b2742e5b414928c5a358c2d)), closes [#240](https://github.com/KinjiKawaguchi/agency/issues/240)
* **web:** draw the org's cost on the same days as a KPI, under it ([#362](https://github.com/KinjiKawaguchi/agency/issues/362)) ([d16e6d1](https://github.com/KinjiKawaguchi/agency/commit/d16e6d1b83860604fdbc407529fe732098a85f15))
* **web:** give a member the node hosts their decisions and devices, and nothing of the owner's ([#361](https://github.com/KinjiKawaguchi/agency/issues/361)) ([d53f375](https://github.com/KinjiKawaguchi/agency/commit/d53f375939c9b06f529f90bc6f1be539ca07d044))
* **web:** hear a held root change in the browser, and word it and a recovery in the page's language ([#402](https://github.com/KinjiKawaguchi/agency/issues/402)) ([6dcccef](https://github.com/KinjiKawaguchi/agency/commit/6dcccef1c4b8072915189a816b8b3a1d73695f68))
* **web:** hold the client's weight and time to the first desk to a budget ([#410](https://github.com/KinjiKawaguchi/agency/issues/410)) ([d4b9702](https://github.com/KinjiKawaguchi/agency/commit/d4b9702bf31229aa14046b9569ae3821c2ca10f6))
* **web:** let the owner enter a KPI's number and read its history ([#356](https://github.com/KinjiKawaguchi/agency/issues/356)) ([56c380c](https://github.com/KinjiKawaguchi/agency/commit/56c380c9a13fbcebe9c1950809d59cc9ba89739c))
* **web:** list the home nodes the node names, with the ones the owner added ([#330](https://github.com/KinjiKawaguchi/agency/issues/330)) ([f5ec0c6](https://github.com/KinjiKawaguchi/agency/commit/f5ec0c6502e46b773ed12e474dbde0e13dae6fbf))
* **web:** list where the node sends notices and let the owner stop one by hand ([#467](https://github.com/KinjiKawaguchi/agency/issues/467)) ([59d8b5c](https://github.com/KinjiKawaguchi/agency/commit/59d8b5c3d3ed7e362ad59233cb96f87c77720c9a))
* **web:** name this browser's device when it registers for push, so removing the device stops its notices ([#455](https://github.com/KinjiKawaguchi/agency/issues/455)) ([e80319a](https://github.com/KinjiKawaguchi/agency/commit/e80319a9003ed6bdac746f7d60d316dd67486314))
* **web:** notify the browser with the app closed, through the node's Web Push ([#333](https://github.com/KinjiKawaguchi/agency/issues/333)) ([cd9e5c8](https://github.com/KinjiKawaguchi/agency/commit/cd9e5c83f6ff8d0b273d59602ed7b9159f84cc8e))
* **web:** read each group's own p95 for an hour from the buckets of its routes ([#421](https://github.com/KinjiKawaguchi/agency/issues/421)) ([cdce1ce](https://github.com/KinjiKawaguchi/agency/commit/cdce1ce2ad919777d03d3f0507c2ad447cd57a05))
* **web:** read the objectives as the generated type, and say when one cannot be told ([#426](https://github.com/KinjiKawaguchi/agency/issues/426)) ([883962c](https://github.com/KinjiKawaguchi/agency/commit/883962c11fe58060de8944b2ba601dadb0f29357)), closes [#240](https://github.com/KinjiKawaguchi/agency/issues/240)
* **web:** remove one of the owner's devices from the Devices screen ([#451](https://github.com/KinjiKawaguchi/agency/issues/451)) ([ed15943](https://github.com/KinjiKawaguchi/agency/commit/ed15943a3c44f6c0c24666b878b65f5b9b09b70b))
* **web:** report how long the owner's calls took to the node ([#491](https://github.com/KinjiKawaguchi/agency/issues/491)) ([6641484](https://github.com/KinjiKawaguchi/agency/commit/66414848c659a6ab632450c43513a3cb52a6d6aa))
* **web:** say a device approval the root's node holds as it is, and show what waits for the root key ([#384](https://github.com/KinjiKawaguchi/agency/issues/384)) ([77b6604](https://github.com/KinjiKawaguchi/agency/commit/77b6604e001faf1822c8efe833b2cfff1c340667))
* **web:** say no title to the operating system when the node hides titles ([#326](https://github.com/KinjiKawaguchi/agency/issues/326)) ([f544bec](https://github.com/KinjiKawaguchi/agency/commit/f544bec4621beb90cf98d9d3abfd8f40c54f5492))
* **web:** say plainly on the Devices card when a browser's notifications are failing ([#387](https://github.com/KinjiKawaguchi/agency/issues/387)) ([e9164e1](https://github.com/KinjiKawaguchi/agency/commit/e9164e1287046d5e4dce80deb0330eabe99c1720))
* **web:** say whom a request is for when the viewer answers for a seat or an external party ([#407](https://github.com/KinjiKawaguchi/agency/issues/407)) ([08fd7dd](https://github.com/KinjiKawaguchi/agency/commit/08fd7ddc4ddf50617a7d13cc4a1cc25e9f8cd2a4))
* **web:** show an org's change history to its owner ([#631](https://github.com/KinjiKawaguchi/agency/issues/631)) ([ae66804](https://github.com/KinjiKawaguchi/agency/commit/ae66804e48e38a3e4df4d4e6c2c9bbde36be8705))
* **web:** show an org's members, devices and seats to its owner ([#614](https://github.com/KinjiKawaguchi/agency/issues/614)) ([36798f7](https://github.com/KinjiKawaguchi/agency/commit/36798f70c9324ee06889e2424beb58c9b7be5ac3))
* **web:** show how each participant spent the period, what it cost, and what they merged ([#376](https://github.com/KinjiKawaguchi/agency/issues/376)) ([8faf089](https://github.com/KinjiKawaguchi/agency/commit/8faf08913a4614241622e5a49f4e912666e8ae12))
* **web:** show the node's own response time to its owner, against the budget ([#395](https://github.com/KinjiKawaguchi/agency/issues/395)) ([989fcab](https://github.com/KinjiKawaguchi/agency/commit/989fcab589e11b33aa2e2be73f66d3ee0a57a199))
* **web:** show the owner's outcomes and how each aim's KPI stood around the day its work shipped ([#345](https://github.com/KinjiKawaguchi/agency/issues/345)) ([7ad8df4](https://github.com/KinjiKawaguchi/agency/commit/7ad8df446bba77cece768bd4725be4f66da21e16))
* **web:** show the weekly budget as the node computes it ([#594](https://github.com/KinjiKawaguchi/agency/issues/594)) ([e640f3e](https://github.com/KinjiKawaguchi/agency/commit/e640f3e62e1045342456ddf0f5253c8f117b08c2))
* **web:** tell the owner at the top of the desk that a recovery of an org was started ([#368](https://github.com/KinjiKawaguchi/agency/issues/368)) ([846c8c1](https://github.com/KinjiKawaguchi/agency/commit/846c8c1b1a01115cf674ce590a6ba922241fb73a))


### Bug fixes

* **apple:** do not remove a device while decisions it signed wait to be sent ([#478](https://github.com/KinjiKawaguchi/agency/issues/478)) ([71ee342](https://github.com/KinjiKawaguchi/agency/commit/71ee34242fcae953c2712589d58c58013ddc1656))
* **apple:** keep a device while kept decisions cannot be read ([#628](https://github.com/KinjiKawaguchi/agency/issues/628)) ([15d2328](https://github.com/KinjiKawaguchi/agency/commit/15d232866a516ff2923d5c52c2d66cdbe6ca97a3))
* **apple:** keep the other screens answered, and a cancelled read from showing as a failure ([#385](https://github.com/KinjiKawaguchi/agency/issues/385)) ([aaf8693](https://github.com/KinjiKawaguchi/agency/commit/aaf86935ba0e387ad302880a10a140db4ca78498))
* **apple:** keep what the node said the viewer is across a key reset ([#476](https://github.com/KinjiKawaguchi/agency/issues/476)) ([01fad60](https://github.com/KinjiKawaguchi/agency/commit/01fad601a95b3469c7e6cb319fb79ebf2346336c))
* **apple:** never replace a key the device already has without the owner's choice ([#481](https://github.com/KinjiKawaguchi/agency/issues/481)) ([f8528a2](https://github.com/KinjiKawaguchi/agency/commit/f8528a2478697b958e1375c2cf580b38656437b8))
* **apple:** read a word of the node that the app does not know as unknown ([#394](https://github.com/KinjiKawaguchi/agency/issues/394)) ([240edce](https://github.com/KinjiKawaguchi/agency/commit/240edce3194ce23ee881901a62893baa23b46df1))
* **apple:** say in the Mac's menu bar when a read failed, and when there is no home node ([#331](https://github.com/KinjiKawaguchi/agency/issues/331)) ([3aa8f26](https://github.com/KinjiKawaguchi/agency/commit/3aa8f268657c3393cb2d72b8980ce525ecf4fc02))
* **apple:** show the labels of a request's relay without the characters that can reorder them ([#425](https://github.com/KinjiKawaguchi/agency/issues/425)) ([2a9ce6c](https://github.com/KinjiKawaguchi/agency/commit/2a9ce6c14c6c2a7e652a09b0eca75856a4a32dda))
* **core:** check intake requests before taking the write lock ([#470](https://github.com/KinjiKawaguchi/agency/issues/470)) ([1f9c7af](https://github.com/KinjiKawaguchi/agency/commit/1f9c7af1e44325078f5b26d3d99b3a2e97f85766))
* **core:** count every origin a node was configured with as that node ([#561](https://github.com/KinjiKawaguchi/agency/issues/561)) ([a29ee7f](https://github.com/KinjiKawaguchi/agency/commit/a29ee7fd7349bc87e443ac4d3f63d787bb33e096))
* **core:** keep the names a team runtime reserves out of new member, seat, and external ids ([#493](https://github.com/KinjiKawaguchi/agency/issues/493)) ([81084d0](https://github.com/KinjiKawaguchi/agency/commit/81084d09f75803db048ddcef73ccbd9c1fce050a))
* **core:** name every claim in the root-sign prompt ([#469](https://github.com/KinjiKawaguchi/agency/issues/469)) ([3bd04e3](https://github.com/KinjiKawaguchi/agency/commit/3bd04e3b134b9f7548a1d3e473b256efba4af470))
* **core:** say whom a remote request is for only from own definition ([#504](https://github.com/KinjiKawaguchi/agency/issues/504)) ([da23f2e](https://github.com/KinjiKawaguchi/agency/commit/da23f2eae42812b5f8354115c08b3f2dcbb9765f))
* **core:** take no envelope made before the team came to this node ([#527](https://github.com/KinjiKawaguchi/agency/issues/527)) ([986a8e2](https://github.com/KinjiKawaguchi/agency/commit/986a8e299e97d0c4a2a8c62def837b4a35e589b8))
* **core:** use an invite only when the root signs or holds the join ([#499](https://github.com/KinjiKawaguchi/agency/issues/499)) ([44bd623](https://github.com/KinjiKawaguchi/agency/commit/44bd6237bf0afad6f95986995bf1fdaec377db4d))
* **node:** accept the app's sign-in only through the universal link on the public host ([#468](https://github.com/KinjiKawaguchi/agency/issues/468)) ([f2b62f1](https://github.com/KinjiKawaguchi/agency/commit/f2b62f1a1d916f2d840a5e8d316fc0121006f5c9))
* **node:** admit only a known identity and refuse encoded path separators and dot segments ([#465](https://github.com/KinjiKawaguchi/agency/issues/465)) ([8fa8978](https://github.com/KinjiKawaguchi/agency/commit/8fa8978360073dd4e011e2baa6ea57dbb251ab83))
* **node:** behind Access, let loopback read but write only through Access ([#327](https://github.com/KinjiKawaguchi/agency/issues/327)) ([d97fa28](https://github.com/KinjiKawaguchi/agency/commit/d97fa2885f85a1fa96880a7fd2ac663c17a9d994))
* **node:** drop a device's push registrations when the account no longer has the device ([#453](https://github.com/KinjiKawaguchi/agency/issues/453)) ([299bab6](https://github.com/KinjiKawaguchi/agency/commit/299bab619eb82f68eff0c1fad41a8ef9a4a151b4))
* **node:** keep only the applied envelope ids an envelope can still be sent within ([#558](https://github.com/KinjiKawaguchi/agency/issues/558)) ([46167c6](https://github.com/KinjiKawaguchi/agency/commit/46167c680ecc7e4d5c00ccc1715ca1d8dab53ed8)), closes [#533](https://github.com/KinjiKawaguchi/agency/issues/533)
* **node:** let a node on a shared machine refuse local clients ([#487](https://github.com/KinjiKawaguchi/agency/issues/487)) ([b7e49e6](https://github.com/KinjiKawaguchi/agency/commit/b7e49e6bb5071ee352b5ea6950f17bcccc0be614))
* **node:** never let added-up measurements wrap around to a small number ([#490](https://github.com/KinjiKawaguchi/agency/issues/490)) ([163b2e6](https://github.com/KinjiKawaguchi/agency/commit/163b2e63848a34bed78b065eea5149b4fff2cdd1))
* **node:** send HSTS on answers through Cloudflare from a node whose url is https ([#509](https://github.com/KinjiKawaguchi/agency/issues/509)) ([e672fb6](https://github.com/KinjiKawaguchi/agency/commit/e672fb671e7c8f990775808e5b4e32551144c5e1))
* **node:** send the texts of a push notice printable ([#523](https://github.com/KinjiKawaguchi/agency/issues/523)) ([0229107](https://github.com/KinjiKawaguchi/agency/commit/0229107fded44b1e0f96523ddca1bd57d4e85d84))
* **node:** send Web Push to a few browsers at once, carry only plain references, and say what `sub` tells the services ([#370](https://github.com/KinjiKawaguchi/agency/issues/370)) ([4db6dda](https://github.com/KinjiKawaguchi/agency/commit/4db6dda7de8c8ece70ab95267b76c10232554833))
* **node:** stop retrying what the outbox can never deliver ([#559](https://github.com/KinjiKawaguchi/agency/issues/559)) ([3e875a5](https://github.com/KinjiKawaguchi/agency/commit/3e875a5c8a8c8ece939338b43011f006cb906d60))
* **node:** take only an uncompressed key as a browser's p256dh ([#319](https://github.com/KinjiKawaguchi/agency/issues/319)) ([676e05b](https://github.com/KinjiKawaguchi/agency/commit/676e05ba2303a052bda02747d91e2ee6ffba0176))
* **node:** try a failed destination again alone, so a phone's success does not forget a browser ([#372](https://github.com/KinjiKawaguchi/agency/issues/372)) ([b9719da](https://github.com/KinjiKawaguchi/agency/commit/b9719da31883df924d1b94950049ba06f08d3ac5)), closes [#284](https://github.com/KinjiKawaguchi/agency/issues/284)
* **proto:** hide the line and paragraph separators and other invisible format characters ([#475](https://github.com/KinjiKawaguchi/agency/issues/475)) ([e248137](https://github.com/KinjiKawaguchi/agency/commit/e24813769d0f809f34a2001504419ea7c20f0dd6))
* **proto:** read buckets that add up past what a count holds as inconsistent ([#463](https://github.com/KinjiKawaguchi/agency/issues/463)) ([fca1613](https://github.com/KinjiKawaguchi/agency/commit/fca1613e4023889894310882aa61209376e45822)), closes [#429](https://github.com/KinjiKawaguchi/agency/issues/429)
* **web:** count the undelivered decisions over all the log the node gives, and say "at least" when it is full ([#521](https://github.com/KinjiKawaguchi/agency/issues/521)) ([cdb3d2c](https://github.com/KinjiKawaguchi/agency/commit/cdb3d2cceb2b76df619f9fb28b96329e51e76420))
* **web:** draw the names of the team's runtime as marks where a hidden character would reorder them, and guard the places ([#422](https://github.com/KinjiKawaguchi/agency/issues/422)) ([fa7a165](https://github.com/KinjiKawaguchi/agency/commit/fa7a1656da1b5e0ddc24bc0ecaa73f4cc3a09ea1))
* **web:** draw what an agent or a person wrote as printable text, and guard every name the runtime gives ([#435](https://github.com/KinjiKawaguchi/agency/issues/435)) ([d48b73b](https://github.com/KinjiKawaguchi/agency/commit/d48b73be16728a8c72a3bf90319d58ae4216dfff))
* **web:** make the outbox e2e wait on the node's record, and send what waits when the page opens ([#472](https://github.com/KinjiKawaguchi/agency/issues/472)) ([220710a](https://github.com/KinjiKawaguchi/agency/commit/220710a42c4027b92df8a14c6703624bee5afeee))
* **web:** print each hidden character of a node's label as a mark ([#417](https://github.com/KinjiKawaguchi/agency/issues/417)) ([8400814](https://github.com/KinjiKawaguchi/agency/commit/84008143caa06c1c57fb5e1806e9613990919371))
* **web:** say "saved, not delivered" for a decision the node holds, not "waiting to send" ([#454](https://github.com/KinjiKawaguchi/agency/issues/454)) ([5845ce3](https://github.com/KinjiKawaguchi/agency/commit/5845ce3debea634f75c69c69f93a2f5cb21682bf))
* **web:** show a skeleton, not a default, until the node has answered ([#317](https://github.com/KinjiKawaguchi/agency/issues/317)) ([0bc7211](https://github.com/KinjiKawaguchi/agency/commit/0bc721195415efeb315909fd0101c0b632466171))


### Performance

* **web:** fetch the pages other than the desk and the devices when they are first opened ([#420](https://github.com/KinjiKawaguchi/agency/issues/420)) ([6959463](https://github.com/KinjiKawaguchi/agency/commit/695946320fd8623b3600f930adc4de26c5011115))


### Refactoring

* **proto:** build an OrgDefinition through its constructor ([#367](https://github.com/KinjiKawaguchi/agency/issues/367)) ([50d158e](https://github.com/KinjiKawaguchi/agency/commit/50d158e3702e02bce793fb312731110996359880)), closes [#365](https://github.com/KinjiKawaguchi/agency/issues/365)
* **web:** look at a feature's options in one place ([#641](https://github.com/KinjiKawaguchi/agency/issues/641)) ([90d2bd3](https://github.com/KinjiKawaguchi/agency/commit/90d2bd3ebb11c193160bc38bf0a6b09795e46a9d))
* **web:** name each call of the client by its route template, and build its path from it ([#440](https://github.com/KinjiKawaguchi/agency/issues/440)) ([8a1dbc0](https://github.com/KinjiKawaguchi/agency/commit/8a1dbc0643be112fcea7f86c81fd00b4d3ce5696))
* **web:** one module per feature for its route, capability, mock answer and guest rule ([#637](https://github.com/KinjiKawaguchi/agency/issues/637)) ([1e7eb5f](https://github.com/KinjiKawaguchi/agency/commit/1e7eb5f368844c4bca9c9483a1f70e0c242a3d99))
* **web:** read the guest's org too, as the node says it, and test that no funding is drawn for a guest ([#393](https://github.com/KinjiKawaguchi/agency/issues/393)) ([de4ce23](https://github.com/KinjiKawaguchi/agency/commit/de4ce23509c7f8558cfc0730f1567b5816e6b0f0))
* **web:** take the generated types for recovery and held root changes, and drop the assumed shapes ([#392](https://github.com/KinjiKawaguchi/agency/issues/392)) ([ae90aaa](https://github.com/KinjiKawaguchi/agency/commit/ae90aaab52ffca8296b0d6c4356def52a1af3a40))


### Documentation

* add a security policy, a threat model and signed releases ([#601](https://github.com/KinjiKawaguchi/agency/issues/601)) ([b520faf](https://github.com/KinjiKawaguchi/agency/commit/b520faf0f52d5933ac6578b261aaeeb2a9cccdd0))
* **apple:** a team on another node shows its seat's requests too, and test it ([#431](https://github.com/KinjiKawaguchi/agency/issues/431)) ([8b2959f](https://github.com/KinjiKawaguchi/agency/commit/8b2959f8c3da60e8301372c3809d59029290303b))
* explain the Web Push subject ([#500](https://github.com/KinjiKawaguchi/agency/issues/500)) ([d498c2f](https://github.com/KinjiKawaguchi/agency/commit/d498c2f0f12db58158e4afded1a3b8d99c47241f))
* give the owner a checklist for notifications with the app closed, and say how to turn them off ([#382](https://github.com/KinjiKawaguchi/agency/issues/382)) ([e4e317c](https://github.com/KinjiKawaguchi/agency/commit/e4e317c0906295100e212d632647f638a9684100))
* name the sending tray's words, and keep queued the node's word ([#452](https://github.com/KinjiKawaguchi/agency/issues/452)) ([d0f372a](https://github.com/KinjiKawaguchi/agency/commit/d0f372a8f414cdbc0112d2c3cadde7171061c48b))
* name the states of a reading, the movement of a number, and the outcomes page ([#339](https://github.com/KinjiKawaguchi/agency/issues/339)) ([7bde6ef](https://github.com/KinjiKawaguchi/agency/commit/7bde6efb754a7f2abdc5c0a95d0158f9f6b00d3b))
* say what a node keeps, for how long, and how to delete it ([#592](https://github.com/KinjiKawaguchi/agency/issues/592)) ([6672007](https://github.com/KinjiKawaguchi/agency/commit/6672007818f9e59b06bcb0a216dc76d1a74ab175))
* say what percent and ratio mean in the KPI units ([#341](https://github.com/KinjiKawaguchi/agency/issues/341)) ([422eb94](https://github.com/KinjiKawaguchi/agency/commit/422eb945538acfa9e4cd42ff6a02ff40b5fb4f5b)), closes [#340](https://github.com/KinjiKawaguchi/agency/issues/340)

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
