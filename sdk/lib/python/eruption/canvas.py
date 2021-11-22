#    This file is part of Eruption.
#
#    Eruption is free software: you can redistribute it and/or modify
#    it under the terms of the GNU General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
#
#    Eruption is distributed in the hope that it will be useful,
#    but WITHOUT ANY WARRANTY; without even the implied warranty of
#    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
#    GNU General Public License for more details.
#
#    You should have received a copy of the GNU General Public License
#    along with Eruption.  If not, see <http://www.gnu.org/licenses/>.


from eruption.color import Color

CANVAS_SIZE = 144 + 36

class Canvas:
    """A canvas that can be submitted to Eruption via an open connection"""

    def __init__(self, *args, **kwargs):
        """Create a canvas and initialize it with a transparent color"""
        self.size = CANVAS_SIZE
        self.data = [Color(0, 0, 0, 0)] * CANVAS_SIZE

    def fill_all(self, color):
        """Paint the canvas with the specified color"""
        for i in range(0, self.size):
            self.data[i] = color
