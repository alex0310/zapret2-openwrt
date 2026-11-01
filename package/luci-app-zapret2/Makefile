include $(TOPDIR)/rules.mk

PKG_NAME:=luci-app-zapret2
PKG_VERSION:=1.0.0
PKG_RELEASE:=1

PKG_MAINTAINER:=alex0310 <https://github.com/alex0310>
PKG_LICENSE:=MIT

include $(INCLUDE_DIR)/package.mk

define Package/luci-app-zapret2
  SECTION:=luci
  CATEGORY:=LuCI
  SUBMENU:=3. Applications
  TITLE:=LuCI support for zapret2
  DEPENDS:=+luci-base +zapret2
  PKGARCH:=all
endef

define Package/luci-app-zapret2/description
 Web interface for managing zapret2 DPI bypass tool.
 Provides configuration UI for strategies, lists, scripts, and blockcheck.
endef

define Build/Compile
endef

define Package/luci-app-zapret2/install
	$(INSTALL_DIR) $(1)/usr/lib/lua/luci/i18n
	$(INSTALL_DIR) $(1)/usr/share/luci/menu.d
	$(INSTALL_DIR) $(1)/usr/share/rpcd/acl.d
	$(INSTALL_DIR) $(1)/usr/share/ucitrack
	$(INSTALL_DIR) $(1)/www/luci-static/resources/view/zapret2
	$(CP) ./files/. $(1)/
endef

$(eval $(call BuildPackage,luci-app-zapret2))
