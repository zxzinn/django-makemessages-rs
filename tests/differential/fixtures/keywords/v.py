from django.utils.translation import gettext as _
a = _("Standard string")
b = mytrans("Custom keyword string")
c = myctx("ctxname", "Custom with context")
d = myplural("one item", "many items", n)
