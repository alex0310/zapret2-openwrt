include $(TOPDIR)/rules.mk

PKG_NAME:=zapret2
PKG_VERSION:=1.0.0
PKG_RELEASE:=1

PKG_SOURCE:=v1.0.2.tar.gz
PKG_SOURCE_URL:=https://github.com/bol-van/zapret2/archive/refs/tags/
PKG_HASH:=b778c9fda24de6d534881ce6c9ff5817ea245163d3b96dc1c1fd491704252e60
PKG_BUILD_DIR:=$(BUILD_DIR)/zapret2-1.0.2

PKG_BUILD_PARALLEL:=1
PKG_MAINTAINER:=bol-van
PKG_LICENSE:=MIT
PKG_LICENSE_FILES:=docs/LICENSE.txt

# LuaJIT 2.1 has no RISC-V backend, so LuaJIT (and OpenResty luajit2)
# cannot be built for riscv64. Use the portable Lua 5.1 library there.
ifneq ($(filter riscv64%,$(ARCH) $(CONFIG_TARGET_ARCH_PACKAGES)),)
  ZAPRET2_LUA_JIT:=0
  ZAPRET2_LUA_CFLAGS:=-I$(STAGING_DIR)/usr/include
  ZAPRET2_LUA_LIB:=-L$(STAGING_DIR)/usr/lib -llua
else
  ZAPRET2_LUA_JIT:=1
  ZAPRET2_LUA_CFLAGS:=-I$(STAGING_DIR)/usr/include/luajit-2.1
  ZAPRET2_LUA_LIB:=-L$(STAGING_DIR)/usr/lib -lluajit-5.1
endif

# Runtime and build dependencies must use Kconfig conditions, because the
# package metadata is dumped (DUMP=1) without reading .config, where ARCH
# and CONFIG_TARGET_ARCH_PACKAGES are not available yet.
PKG_BUILD_DEPENDS:=!riscv64:luajit riscv64:lua libcap

include $(INCLUDE_DIR)/package.mk

# Prebuilt Rust binaries are selected per package architecture. Most ARM
# variants need separate binaries because their CPU, FPU and float ABI
# differ; mipsel_24kc/mipsel_74kc share the default (no suffix) binaries.
# ARCH_PACKAGES is the unquoted package architecture (CONFIG_TARGET_ARCH_PACKAGES
# keeps the quotes from .config, so it cannot be matched directly).
ifneq ($(filter aarch64%,$(ARCH) $(ARCH_PACKAGES)),)
  ZAPRET2_RUST_SUFFIX:=-aarch64
else ifneq ($(filter x86_64%,$(ARCH) $(ARCH_PACKAGES)),)
  ZAPRET2_RUST_SUFFIX:=-x86_64
else ifneq ($(filter riscv64%,$(ARCH) $(ARCH_PACKAGES)),)
  ZAPRET2_RUST_SUFFIX:=-riscv64
else ifneq ($(filter mips64_octeonplus%,$(ARCH) $(ARCH_PACKAGES)),)
  ZAPRET2_RUST_SUFFIX:=-mips64
else ifneq ($(filter mips_mips32%,$(ARCH) $(ARCH_PACKAGES)),)
  ZAPRET2_RUST_SUFFIX:=-mips32
else ifneq ($(filter mipsel%,$(ARCH) $(ARCH_PACKAGES)),)
  ZAPRET2_RUST_SUFFIX:=
else ifneq ($(filter mips%,$(ARCH) $(ARCH_PACKAGES)),)
  ZAPRET2_RUST_SUFFIX:=-mips
else ifneq ($(filter arm_cortex-a9_vfpv3-d16%,$(ARCH) $(ARCH_PACKAGES)),)
  ZAPRET2_RUST_SUFFIX:=-cortexa9-vfpv3d16
else ifneq ($(filter arm_cortex-a9_neon%,$(ARCH) $(ARCH_PACKAGES)),)
  ZAPRET2_RUST_SUFFIX:=-cortexa9-neon
else ifneq ($(filter arm_cortex-a9%,$(ARCH) $(ARCH_PACKAGES)),)
  ZAPRET2_RUST_SUFFIX:=-cortexa9
else ifneq ($(filter arm_xscale%,$(ARCH) $(ARCH_PACKAGES)),)
  ZAPRET2_RUST_SUFFIX:=-xscale
else ifneq ($(filter arm_fa526%,$(ARCH) $(ARCH_PACKAGES)),)
  ZAPRET2_RUST_SUFFIX:=-fa526
else
  ZAPRET2_RUST_SUFFIX:=
endif

ifneq ($(findstring 25.12,$(CONFIG_VERSION_REPO)),)
  ZAPRET2_RUST_DIR:=./files$(ZAPRET2_RUST_SUFFIX)-25.12
else
  ZAPRET2_RUST_DIR:=./files$(ZAPRET2_RUST_SUFFIX)
endif

define Package/zapret2
  SECTION:=net
  CATEGORY:=Network
  SUBMENU:=Web Servers/Proxies
  TITLE:=DPI bypass tool using nfqws2
  URL:=https://github.com/bol-van/zapret2
  DEPENDS:=+libnetfilter-queue +libnfnetlink +libmnl +zlib +!riscv64:luajit +riscv64:liblua \
	+kmod-nft-queue +kmod-nfnetlink-queue +kmod-nf-conntrack \
	+nftables +gzip +curl +ncat +ca-bundle +uci
endef

define Package/zapret2/description
 zapret2 is a DPI bypass tool using nfqws2.
 Includes nfqws2/mdig built from upstream v1.0.2, Lua scripts, firewall
 scripts, a Rust management CLI and a Rust strategy finder.
endef

define Build/Compile
	$(MAKE) -C $(PKG_BUILD_DIR)/nfq2 \
		CC="$(TARGET_CC)" \
		CFLAGS="$(TARGET_CFLAGS) $(TARGET_CPPFLAGS)" \
		LDFLAGS="$(TARGET_LDFLAGS)" \
		LUA_JIT=$(ZAPRET2_LUA_JIT) \
		LUA_CFLAGS="$(ZAPRET2_LUA_CFLAGS)" \
		LUA_LIB="$(ZAPRET2_LUA_LIB)" \
		nfqws2
	$(MAKE) -C $(PKG_BUILD_DIR)/mdig \
		CC="$(TARGET_CC)" \
		CFLAGS="$(TARGET_CFLAGS) $(TARGET_CPPFLAGS)" \
		LDFLAGS="$(TARGET_LDFLAGS)" \
		mdig
endef

define Package/zapret2/install
	$(INSTALL_DIR) $(1)/opt/zapret2
	$(CP) ./files/. $(1)/
	$(INSTALL_BIN) $(PKG_BUILD_DIR)/nfq2/nfqws2 $(1)/opt/zapret2/nfq2/nfqws2
	$(INSTALL_BIN) $(PKG_BUILD_DIR)/mdig/mdig $(1)/opt/zapret2/mdig/mdig
	$(INSTALL_BIN) $(ZAPRET2_RUST_DIR)/usr/bin/zapret2 $(1)/usr/bin/zapret2
	$(INSTALL_BIN) $(ZAPRET2_RUST_DIR)/opt/zapret2/zapret2-finder $(1)/opt/zapret2/zapret2-finder
	$(INSTALL_BIN) ./files/etc/init.d/zapret2 $(1)/etc/init.d/zapret2
	$(INSTALL_CONF) ./files/etc/config/zapret2 $(1)/etc/config/zapret2
	$(INSTALL_CONF) ./files/opt/zapret2/config $(1)/opt/zapret2/config
endef

define Package/zapret2/conffiles
/etc/config/zapret2
/opt/zapret2/config
/opt/zapret2/ipset/clear_lists.sh
/opt/zapret2/ipset/create_ipset.sh
/opt/zapret2/ipset/def.sh
/opt/zapret2/ipset/get_antifilter_allyouneed.sh
/opt/zapret2/ipset/get_antifilter_ip.sh
/opt/zapret2/ipset/get_antifilter_ipresolve.sh
/opt/zapret2/ipset/get_antifilter_ipsmart.sh
/opt/zapret2/ipset/get_antifilter_ipsum.sh
/opt/zapret2/ipset/get_antizapret_domains.sh
/opt/zapret2/ipset/get_config.sh
/opt/zapret2/ipset/get_exclude.sh
/opt/zapret2/ipset/get_ipban.sh
/opt/zapret2/ipset/get_reestr_hostlist.sh
/opt/zapret2/ipset/get_reestr_preresolved.sh
/opt/zapret2/ipset/get_reestr_preresolved_smart.sh
/opt/zapret2/ipset/get_reestr_resolvable_domains.sh
/opt/zapret2/ipset/get_reestr_resolve.sh
/opt/zapret2/ipset/get_refilter_domains.sh
/opt/zapret2/ipset/get_refilter_ipsum.sh
/opt/zapret2/ipset/get_user.sh
/opt/zapret2/ipset/zapret-ip-exclude.txt
/opt/zapret2/ipset/zapret-ip-exclude6.txt
/opt/zapret2/ipset/zapret_hosts_user_exclude.txt
/opt/zapret2/ipset/zapret_hosts_user_exclude.txt.default
/opt/zapret2/ipset/zapret_hosts_youtube.txt
/opt/zapret2/ipset/zapret_hosts_youtube.txt.default
endef

$(eval $(call BuildPackage,zapret2))
