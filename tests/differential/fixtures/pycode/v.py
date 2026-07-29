from django.utils.translation import gettext as _, ngettext, pgettext, npgettext
a = _("py simple")
b = _("py implicit" " concat")
c = ngettext("one thing", "many things", n)
d = pgettext("pyctx", "py with context")
e = npgettext("pyctx2", "np one", "np many", n)
f = _("""triple quoted""")
