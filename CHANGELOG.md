# Changelog

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
