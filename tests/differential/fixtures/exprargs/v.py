from django.utils.translation import gettext as _

a = _(getattr(model._meta, 'verbose_name', label))
b = _(some_variable)
c = _(fn('nested literal'))
d = _("a real string")
e = _("implicit" " concat")
