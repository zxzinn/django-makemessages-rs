from django.utils.translation import gettext as g, pgettext_lazy as pl, ngettext as ng
a = g("Aliased gettext")
b = pl("actx", "Aliased pgettext_lazy")
c = ng("alias one", "alias many", n)
