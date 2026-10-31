The Widget API
==============

This page is written in reStructuredText and served as it is. The site converts it when it builds
the page, so the same properties file serves Markdown and reStructuredText side by side.

.. |name| replace:: **Widget**

Overview
--------

The |name| library draws small components.
It is described in the `reference <https://example.org/reference>`_ and in the notes [1]_.

.. note::
   A note becomes an alert, with the same kind.

Options
-------

-v, --verbose   Print what is done.
--width=N       Set the width of the output.

Terms
-----

draw
    Put a component on a page.

    The component keeps its state in the browser.

hide
    Take it off the page.

Sizes
-----

=======  ======  ====================
Name     Width   Use
=======  ======  ====================
small    80      A phone
medium   100     A laptop
large    140     A wide screen
=======  ======  ====================

Example
-------

.. code-block:: python

   from widgets import Tabs

   Tabs("install").draw()

.. [1] A footnote stays a footnote, and goes to the end of the page.
